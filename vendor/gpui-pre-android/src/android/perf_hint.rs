//! Performance hints (ADPF, `APerformanceHint`, Android 13+): the system is
//! told which thread draws the frames, how long a frame may take and how
//! long each one took, and raises or lowers the CPU frequency (and picks the
//! cores) to fit. Without it the scheduler sees a native thread with short
//! bursts of work and leaves it on slow cores at a low frequency: on a
//! mid-range phone a frame that needs 14 ms of work was drawn at 20 to 30
//! frames a second.
//!
//! The functions arrived in API 33 and the library is built for API 26, so
//! they are looked up at run time; older systems get no session.
//!
//! Not in gpui-mobile.

use std::ffi::{c_int, c_void, CStr};
use std::time::Duration;

type GetManager = unsafe extern "C" fn() -> *mut c_void;
type CreateSession = unsafe extern "C" fn(*mut c_void, *const i32, usize, i64) -> *mut c_void;
type UpdateTarget = unsafe extern "C" fn(*mut c_void, i64) -> c_int;
type ReportActual = unsafe extern "C" fn(*mut c_void, i64) -> c_int;

/// Android 15: the work duration split into its CPU and GPU parts, which
/// lets the system raise the GPU's frequency too.
struct WorkDuration {
    duration: *mut c_void,
    set_start: unsafe extern "C" fn(*mut c_void, i64),
    set_total: unsafe extern "C" fn(*mut c_void, i64),
    set_cpu: unsafe extern "C" fn(*mut c_void, i64),
    set_gpu: unsafe extern "C" fn(*mut c_void, i64),
    report: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
}

impl WorkDuration {
    // the fields give each transmute its target type
    #[allow(clippy::missing_transmute_annotations)]
    fn new() -> Option<Self> {
        // SAFETY: the NDK functions of those names (API 35)
        unsafe {
            let create: unsafe extern "C" fn() -> *mut c_void =
                std::mem::transmute(lookup(c"AWorkDuration_create")?);
            let work = Self {
                set_start: std::mem::transmute::<*mut c_void, _>(lookup(
                    c"AWorkDuration_setWorkPeriodStartTimestampNanos",
                )?),
                set_total: std::mem::transmute::<*mut c_void, _>(lookup(
                    c"AWorkDuration_setActualTotalDurationNanos",
                )?),
                set_cpu: std::mem::transmute::<*mut c_void, _>(lookup(
                    c"AWorkDuration_setActualCpuDurationNanos",
                )?),
                set_gpu: std::mem::transmute::<*mut c_void, _>(lookup(
                    c"AWorkDuration_setActualGpuDurationNanos",
                )?),
                report: std::mem::transmute::<*mut c_void, _>(lookup(
                    c"APerformanceHint_reportActualWorkDuration2",
                )?),
                duration: create(),
            };
            (!work.duration.is_null()).then_some(work)
        }
    }
}

pub(crate) struct PerfHint {
    session: *mut c_void,
    update_target: UpdateTarget,
    report_actual: ReportActual,
    work: Option<WorkDuration>,
    target: Duration,
}

fn lookup(name: &CStr) -> Option<*mut c_void> {
    // SAFETY: a symbol lookup in the libraries already loaded (libandroid)
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    (!symbol.is_null()).then_some(symbol)
}

impl PerfHint {
    /// A session for the calling thread (the one that draws), aiming at
    /// `target` per frame; `None` before Android 13 or when the device has
    /// no hint manager.
    pub(crate) fn for_current_thread(target: Duration) -> Option<Self> {
        // SAFETY: the symbols are the NDK functions of those names
        // (android/performance_hint.h); the session is used on this thread
        unsafe {
            let get_manager: GetManager =
                std::mem::transmute(lookup(c"APerformanceHint_getManager")?);
            let create_session: CreateSession =
                std::mem::transmute(lookup(c"APerformanceHint_createSession")?);
            let update_target: UpdateTarget =
                std::mem::transmute(lookup(c"APerformanceHint_updateTargetWorkDuration")?);
            let report_actual: ReportActual =
                std::mem::transmute(lookup(c"APerformanceHint_reportActualWorkDuration")?);
            let manager = get_manager();
            if manager.is_null() {
                return None;
            }
            let thread = libc::gettid();
            let session = create_session(manager, &thread, 1, target.as_nanos() as i64);
            if session.is_null() {
                return None;
            }
            Some(Self {
                session,
                update_target,
                report_actual,
                work: WorkDuration::new(),
                target,
            })
        }
    }

    /// Whether the system takes the GPU's share of a frame (Android 15).
    pub(crate) fn reports_gpu(&self) -> bool {
        self.work.is_some()
    }

    /// One frame took `actual`, `gpu` of it waiting for the GPU; `target`
    /// is what a frame may take now (the display's refresh period).
    pub(crate) fn frame(&mut self, actual: Duration, gpu: Duration, target: Duration) {
        // SAFETY: the session of `for_current_thread`, on its thread
        unsafe {
            if target != self.target && !target.is_zero() {
                self.target = target;
                (self.update_target)(self.session, target.as_nanos() as i64);
            }
            // the system rejects durations of zero
            let total = actual.as_nanos().max(1) as i64;
            match &self.work {
                Some(work) => {
                    let gpu = (gpu.as_nanos() as i64).clamp(0, total - 1);
                    let mut now = std::mem::zeroed::<libc::timespec>();
                    libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now);
                    let now = now.tv_sec as i64 * 1_000_000_000 + now.tv_nsec as i64;
                    (work.set_start)(work.duration, now - total);
                    (work.set_total)(work.duration, total);
                    (work.set_cpu)(work.duration, total - gpu);
                    (work.set_gpu)(work.duration, gpu);
                    (work.report)(self.session, work.duration);
                }
                None => {
                    (self.report_actual)(self.session, total);
                }
            }
        }
    }
}
