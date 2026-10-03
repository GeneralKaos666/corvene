//! OS notifications (GHD `vendor/desktop-notifications`:
//! `src/mac/GHDesktopNotificationsManager.m`, `lib/index.ts`):
//! `UNUserNotificationCenter` on macOS.
//!
//! - permission: `getNotificationsPermission` / `requestNotificationsPermission`
//!   / `getNotificationSettingsUrl`;
//! - posting: `showNotificationWithIdentifier:title:body:userInfo:` (asks for
//!   authorization first, then adds an immediate request with the default
//!   sound);
//! - clicks: a `UNUserNotificationCenterDelegate` (`CorveneNotificationDelegate`)
//!   hands the identifier and the `userInfo` payload to the handler installed
//!   with [`install_click_handler`], and lets notifications show while
//!   Corvene is frontmost (`willPresentNotification:`).
//!
//! The notification centre only exists for a process started from an app
//! bundle (it raises an Objective-C exception otherwise), so every entry
//! point checks for a bundle identifier first and reports
//! [`NotificationPermission::Unsupported`] / does nothing without one.
//!
//! Linux: GHD has no `desktop-notifications` backend there
//! (`main-process/notifications.ts`: "notifications not currently
//! supported") and falls back to the HTML5 `Notification` API
//! (`lib/notifications/show-notification.ts`), which Chromium posts to
//! `org.freedesktop.Notifications` with a `default` action; a click focuses
//! the window and runs the callback. Corvene posts the same way
//! (`notify-rust`) and hands clicks on the `default` action to the click
//! handler. There is no permission to ask for, so [`permission`] reports
//! [`NotificationPermission::Unsupported`], which hides the permission hints
//! as GHD's `supportsNotifications() === false` does; clicks on
//! notifications of an earlier session are not delivered (the D-Bus
//! connection that would receive them is gone).
//!
//! Windows: WinRT toasts under the AppUserModelID of the installer's Start
//! menu shortcut, with a COM toast activator (`win` below) whose server is
//! Corvene itself, so a click reaches the running Corvene or starts one,
//! with the identifier and payload the toast carried; clicks on
//! notifications of an earlier session are delivered like macOS's. Windows
//! shows toasts without asking ([`NotificationPermission::Granted`]).
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

/// GHD `NotificationPermission`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationPermission {
    /// Not asked yet (`UNAuthorizationStatusNotDetermined`).
    Default,
    Granted,
    Denied,
    /// No notification centre (not running from an app bundle).
    Unsupported,
}

/// A click on one of Corvene's notifications (GHD `notification-event`
/// `click`, `id`, `userInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationClick {
    pub identifier: String,
    /// The `payload` string posted with [`show`], if any.
    pub payload: Option<String>,
}

/// Why a notification was not shown.
#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("notifications need Corvene to run from its app bundle")]
    Unsupported,
    /// With the centre's error, when it gave one.
    #[error("permission to display notifications wasn't granted{}", .0.as_deref().map(|e| format!(": {e}")).unwrap_or_default())]
    NotGranted(Option<String>),
    #[error("could not post the notification: {0}")]
    Post(String),
}

/// Key of the payload string in the notification's `userInfo`.
#[cfg(target_os = "macos")]
const PAYLOAD_KEY: &str = "corvene-payload";

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::CStr;
    use std::sync::OnceLock;

    use block::Block;
    use objc::declare::ClassDecl;
    use objc::runtime::{BOOL, Class, NO, Object, Protocol, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    use super::NotificationClick;

    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}

    pub type ClickHandler = Box<dyn Fn(NotificationClick) + Send + Sync>;
    pub static CLICK_HANDLER: OnceLock<ClickHandler> = OnceLock::new();

    /// `[NSBundle mainBundle].bundleIdentifier != nil`: the centre exists.
    pub fn has_bundle() -> bool {
        // SAFETY: class messages on NSBundle; both may return nil, checked.
        unsafe {
            let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
            if bundle.is_null() {
                return false;
            }
            let identifier: *mut Object = msg_send![bundle, bundleIdentifier];
            !identifier.is_null()
        }
    }

    /// `[UNUserNotificationCenter currentNotificationCenter]`, when there is one.
    pub fn center() -> Option<*mut Object> {
        if !has_bundle() {
            return None;
        }
        // SAFETY: only reached from a bundled process (see `has_bundle`).
        let center: *mut Object =
            unsafe { msg_send![class!(UNUserNotificationCenter), currentNotificationCenter] };
        (!center.is_null()).then_some(center)
    }

    /// A +1 retained `NSString` (the caller releases it).
    pub unsafe fn ns_string(text: &str) -> *mut Object {
        // NSUTF8StringEncoding
        const UTF8: u64 = 4;
        // SAFETY: `initWithBytes:length:encoding:` copies the bytes.
        unsafe {
            let s: *mut Object = msg_send![class!(NSString), alloc];
            msg_send![s, initWithBytes: text.as_ptr() length: text.len() encoding: UTF8]
        }
    }

    /// The UTF-8 contents of an `NSString` (`None` for nil).
    pub unsafe fn rust_string(s: *mut Object) -> Option<String> {
        if s.is_null() {
            return None;
        }
        // SAFETY: `UTF8String` is valid while `s` lives; copied right away.
        unsafe {
            let ptr: *const std::os::raw::c_char = msg_send![s, UTF8String];
            (!ptr.is_null()).then(|| CStr::from_ptr(ptr).to_string_lossy().into_owned())
        }
    }

    /// `userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:`
    extern "C" fn did_receive(
        _this: &Object,
        _sel: Sel,
        _center: *mut Object,
        response: *mut Object,
        completion: *mut Object,
    ) {
        // blocks are objects; the method's type encoding uses `id`
        let completion = completion as *mut Block<(), ()>;
        // SAFETY: `response` is the framework's live UNNotificationResponse;
        // every step may be nil and is checked. The completion block is
        // called exactly once, as the delegate contract requires.
        unsafe {
            let click = (!response.is_null()).then(|| {
                let notification: *mut Object = msg_send![response, notification];
                let request: *mut Object = msg_send![notification, request];
                let identifier: *mut Object = msg_send![request, identifier];
                let content: *mut Object = msg_send![request, content];
                let user_info: *mut Object = msg_send![content, userInfo];
                let payload = if user_info.is_null() {
                    None
                } else {
                    let key = ns_string(super::PAYLOAD_KEY);
                    let value: *mut Object = msg_send![user_info, objectForKey: key];
                    let _: () = msg_send![key, release];
                    rust_string(value)
                };
                NotificationClick {
                    identifier: rust_string(identifier).unwrap_or_default(),
                    payload,
                }
            });
            if let (Some(click), Some(handler)) = (click, CLICK_HANDLER.get()) {
                handler(click);
            }
            if !completion.is_null() {
                (*completion).call(());
            }
        }
    }

    /// `userNotificationCenter:willPresentNotification:withCompletionHandler:`:
    /// show the banner even while Corvene is frontmost (GHD passes alert,
    /// badge and sound; `alert` is `banner | list` since macOS 11, and
    /// Catalina only knows `alert`).
    extern "C" fn will_present(
        _this: &Object,
        _sel: Sel,
        _center: *mut Object,
        _notification: *mut Object,
        completion: *mut Object,
    ) {
        let completion = completion as *mut Block<(u64,), ()>;
        // UNNotificationPresentationOptionBadge | Sound, then List | Banner
        // or the Alert they replaced
        let options: u64 = if macos_at_least(11, 0) {
            1 | 2 | 8 | 16
        } else {
            1 | 2 | 4
        };
        if !completion.is_null() {
            // SAFETY: the framework's completion block, called once.
            unsafe { (*completion).call((options,)) };
        }
    }

    /// `[NSProcessInfo.processInfo isOperatingSystemAtLeastVersion:]`
    fn macos_at_least(major: isize, minor: isize) -> bool {
        #[repr(C)]
        struct NSOperatingSystemVersion {
            major: isize,
            minor: isize,
            patch: isize,
        }
        let version = NSOperatingSystemVersion {
            major,
            minor,
            patch: 0,
        };
        // SAFETY: the process info singleton always exists; the argument is
        // the struct the selector takes by value.
        unsafe {
            let info: *mut Object = msg_send![class!(NSProcessInfo), processInfo];
            let at_least: BOOL = msg_send![info, isOperatingSystemAtLeastVersion: version];
            at_least != NO
        }
    }

    /// One `CorveneNotificationDelegate` for the process (the centre holds
    /// its delegate weakly, so the instance is never released).
    pub fn delegate() -> *mut Object {
        static DELEGATE: OnceLock<usize> = OnceLock::new();
        *DELEGATE.get_or_init(|| {
            // SAFETY: the class is declared once (guarded by the OnceLock)
            // with methods whose signatures match the protocol's selectors.
            unsafe {
                let class: &'static Class = match ClassDecl::new(
                    "CorveneNotificationDelegate",
                    class!(NSObject),
                ) {
                    Some(mut decl) => {
                        if let Some(protocol) = Protocol::get("UNUserNotificationCenterDelegate") {
                            decl.add_protocol(protocol);
                        }
                        decl.add_method(
                            sel!(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:),
                            did_receive
                                as extern "C" fn(
                                    &Object,
                                    Sel,
                                    *mut Object,
                                    *mut Object,
                                    *mut Object,
                                ),
                        );
                        decl.add_method(
                            sel!(userNotificationCenter:willPresentNotification:withCompletionHandler:),
                            will_present
                                as extern "C" fn(
                                    &Object,
                                    Sel,
                                    *mut Object,
                                    *mut Object,
                                    *mut Object,
                                ),
                        );
                        decl.register()
                    }
                    None => class!(CorveneNotificationDelegate),
                };
                let instance: *mut Object = msg_send![class, new];
                instance as usize
            }
        }) as *mut Object
    }
}

/// The current authorization status, asked synchronously (with a short
/// timeout: the centre answers on its own queue).
#[cfg(target_os = "macos")]
pub fn permission() -> NotificationPermission {
    use std::sync::mpsc;
    use std::time::Duration;

    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let Some(center) = mac::center() else {
        return NotificationPermission::Unsupported;
    };
    let (tx, rx) = mpsc::channel::<i64>();
    // SAFETY: message sends on the shared notification centre; the block
    // copies `tx` and is invoked at most once by the framework.
    unsafe {
        let block = ConcreteBlock::new(move |settings: *mut Object| {
            let status: i64 = if settings.is_null() {
                -1
            } else {
                msg_send![settings, authorizationStatus]
            };
            let _ = tx.send(status);
        });
        let block = block.copy();
        let _: () = msg_send![center, getNotificationSettingsWithCompletionHandler: &*block];
    }
    match rx.recv_timeout(Duration::from_secs(2)) {
        // UNAuthorizationStatus: notDetermined 0, denied 1, authorized 2,
        // provisional 3, ephemeral 4
        Ok(0) => NotificationPermission::Default,
        Ok(1) => NotificationPermission::Denied,
        Ok(2..=4) => NotificationPermission::Granted,
        _ => NotificationPermission::Unsupported,
    }
}

/// UNAuthorizationOptionBadge | Sound | Alert
#[cfg(target_os = "macos")]
const AUTHORIZATION_OPTIONS: u64 = 1 | 2 | 4;

/// Ask the user (the system prompt is asynchronous; poll [`permission`] after).
#[cfg(target_os = "macos")]
pub fn request_permission() {
    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let Some(center) = mac::center() else {
        return;
    };
    // SAFETY: as above; the completion block ignores its arguments.
    unsafe {
        let block = ConcreteBlock::new(move |_granted: bool, _error: *mut Object| {});
        let block = block.copy();
        let _: () = msg_send![center, requestAuthorizationWithOptions: AUTHORIZATION_OPTIONS completionHandler: &*block];
    }
}

/// GHD `showNotification`: post `title` / `body` now, with `payload` in the
/// `userInfo` so a click (even in a later session) can hand it back.
/// Authorization is requested first, as GHD does; `done` runs on the
/// centre's queue with the outcome.
#[cfg(target_os = "macos")]
pub fn show(
    identifier: &str,
    title: &str,
    body: &str,
    payload: Option<&str>,
    done: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) {
    use std::sync::{Arc, Mutex};

    use block::ConcreteBlock;
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    let Some(center) = mac::center() else {
        done(Err(NotificationError::Unsupported));
        return;
    };
    type Done = Box<dyn FnOnce(Result<(), NotificationError>) + Send>;
    let done: Arc<Mutex<Option<Done>>> = Arc::new(Mutex::new(Some(Box::new(done))));
    let finish = move |done: &Arc<Mutex<Option<Done>>>, result| {
        if let Some(done) = done.lock().ok().and_then(|mut d| d.take()) {
            done(result);
        }
    };
    // SAFETY: content and request are built on this thread and retained by
    // the request / centre; the NSStrings created here are released once
    // handed over. The blocks own clones of `done` and run at most once.
    unsafe {
        let content: *mut Object = msg_send![class!(UNMutableNotificationContent), new];
        let ns_title = mac::ns_string(title);
        let ns_body = mac::ns_string(body);
        let _: () = msg_send![content, setTitle: ns_title];
        let _: () = msg_send![content, setBody: ns_body];
        let _: () = msg_send![ns_title, release];
        let _: () = msg_send![ns_body, release];
        let sound: *mut Object = msg_send![class!(UNNotificationSound), defaultSound];
        let _: () = msg_send![content, setSound: sound];
        if let Some(payload) = payload {
            let key = mac::ns_string(PAYLOAD_KEY);
            let value = mac::ns_string(payload);
            let user_info: *mut Object =
                msg_send![class!(NSDictionary), dictionaryWithObject: value forKey: key];
            let _: () = msg_send![content, setUserInfo: user_info];
            let _: () = msg_send![key, release];
            let _: () = msg_send![value, release];
        }
        let ns_identifier = mac::ns_string(identifier);
        let nil: *mut Object = std::ptr::null_mut();
        let request: *mut Object = msg_send![class!(UNNotificationRequest), requestWithIdentifier: ns_identifier content: content trigger: nil];
        let _: () = msg_send![ns_identifier, release];
        let _: () = msg_send![content, release];
        // retained until the authorization answer adds it
        let request: *mut Object = msg_send![request, retain];
        let request = request as usize;
        let center_ptr = center as usize;

        let authorized = ConcreteBlock::new(move |granted: BOOL, error: *mut Object| {
            let request = request as *mut Object;
            if granted == NO || !error.is_null() {
                let _: () = msg_send![request, release];
                let reason = if error.is_null() {
                    None
                } else {
                    let description: *mut Object = msg_send![error, localizedDescription];
                    mac::rust_string(description)
                };
                finish(&done, Err(NotificationError::NotGranted(reason)));
                return;
            }
            let done = done.clone();
            let added = ConcreteBlock::new(move |error: *mut Object| {
                let result = if error.is_null() {
                    Ok(())
                } else {
                    let description: *mut Object = msg_send![error, localizedDescription];
                    Err(NotificationError::Post(
                        mac::rust_string(description).unwrap_or_default(),
                    ))
                };
                finish(&done, result);
            });
            let added = added.copy();
            let center = center_ptr as *mut Object;
            let _: () =
                msg_send![center, addNotificationRequest: request withCompletionHandler: &*added];
            let _: () = msg_send![request, release];
        });
        let authorized = authorized.copy();
        let _: () = msg_send![center, requestAuthorizationWithOptions: AUTHORIZATION_OPTIONS completionHandler: &*authorized];
    }
}

/// GHD `onNotificationEvent` + `configureNotificationsDelegate`: make
/// Corvene the centre's delegate and send every click to `handler`
/// (called on whatever queue the centre delivers on). Install once, early
/// at launch, so a click that starts Corvene is delivered too.
#[cfg(target_os = "macos")]
pub fn install_click_handler(handler: impl Fn(NotificationClick) + Send + Sync + 'static) {
    use objc::{msg_send, sel, sel_impl};

    if mac::CLICK_HANDLER.set(Box::new(handler)).is_err() {
        return;
    }
    let Some(center) = mac::center() else {
        return;
    };
    let delegate = mac::delegate();
    // SAFETY: the delegate instance lives for the whole process.
    unsafe {
        let _: () = msg_send![center, setDelegate: delegate];
    }
}

#[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
mod linux {
    use std::sync::OnceLock;

    use super::NotificationClick;

    pub type ClickHandler = Box<dyn Fn(NotificationClick) + Send + Sync>;
    pub static CLICK_HANDLER: OnceLock<ClickHandler> = OnceLock::new();

    /// The action key the server invokes for a click on the body.
    pub const DEFAULT_ACTION: &str = "default";
}

/// No permission model on Linux (see the module docs).
#[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
pub fn permission() -> NotificationPermission {
    NotificationPermission::Unsupported
}

#[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
pub fn request_permission() {}

/// Post to `org.freedesktop.Notifications` like Chromium's HTML5
/// notifications: app name, summary, body and a `default` action; the
/// desktop entry hint lets the shell show Corvene's icon and name. A thread
/// waits for the notification to be clicked or closed; a click goes to the
/// installed click handler with `identifier` and `payload`.
#[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
pub fn show(
    identifier: &str,
    title: &str,
    body: &str,
    payload: Option<&str>,
    done: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) {
    let mut notification = notify_rust::Notification::new();
    notification
        .appname(crate::paths::APP_NAME)
        .summary(title)
        .body(body)
        .icon(crate::BUNDLE_ID)
        .hint(notify_rust::Hint::DesktopEntry(crate::BUNDLE_ID.into()))
        .action(linux::DEFAULT_ACTION, linux::DEFAULT_ACTION);
    let click = NotificationClick {
        identifier: identifier.to_string(),
        payload: payload.map(str::to_string),
    };
    // connecting to the bus and waiting for the action both block
    let spawned = std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || match notification.show() {
            Ok(handle) => {
                done(Ok(()));
                handle.wait_for_action(|action| {
                    if action == linux::DEFAULT_ACTION
                        && let Some(handler) = linux::CLICK_HANDLER.get()
                    {
                        handler(click);
                    }
                });
            }
            Err(err) => done(Err(NotificationError::Post(err.to_string()))),
        });
    if let Err(err) = spawned {
        tracing::warn!(%err, "could not start the notification thread");
    }
}

/// GHD `onNotificationEvent`: every click on a Corvene notification goes to
/// `handler` (on the notification's waiting thread).
#[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
pub fn install_click_handler(handler: impl Fn(NotificationClick) + Send + Sync + 'static) {
    let _ = linux::CLICK_HANDLER.set(Box::new(handler));
}

/// Windows shows toasts without asking; Settings › System › Notifications
/// can turn them off, which a program is not told.
#[cfg(windows)]
pub fn permission() -> NotificationPermission {
    NotificationPermission::Granted
}

#[cfg(windows)]
pub fn request_permission() {}

// Windows: WinRT toasts (`ToastNotificationManager`) under Corvene's
// AppUserModelID, which an installed Corvene has through its Start menu
// shortcut (`packaging/windows/corvene.iss`). The shortcut also names a
// toast activator CLSID whose COM server is this program: a click calls
// `INotificationActivationCallback::Activate` in the running Corvene, or
// starts one (with `-Embedding`) and calls it there, with the `launch`
// argument the toast carried (identifier and payload), so clicks on
// notifications of an earlier session open their dialog like macOS's. A
// build that is not installed has no identity of its own, and Windows only
// shows toasts of a known one, so its toasts go out under PowerShell's and
// clicks reach it through the toast's in-process `Activated` event only
// while it runs.
#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::sync::Mutex;

    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::Notifications::{
        ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
    };
    use windows::Win32::Foundation::CLASS_E_NOAGGREGATION;
    use windows::Win32::System::Com::{
        CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED, CoInitializeEx, CoRegisterClassObject,
        IClassFactory, IClassFactory_Impl, REGCLS_MULTIPLEUSE,
    };
    use windows::Win32::UI::Notifications::{
        INotificationActivationCallback, INotificationActivationCallback_Impl,
        NOTIFICATION_USER_INPUT_DATA,
    };
    use windows::core::{BOOL, GUID, HSTRING, IInspectable, IUnknown, Interface, PCWSTR, Ref};

    use super::NotificationClick;

    pub type ClickHandler = Box<dyn Fn(NotificationClick) + Send + Sync>;

    /// The click handler, and the clicks that arrived before it was
    /// installed (the click that started this process comes in while
    /// Corvene is still launching).
    pub static CLICKS: Mutex<(Option<ClickHandler>, Vec<NotificationClick>)> =
        Mutex::new((None, Vec::new()));

    /// The identity toasts of a build that is not installed go out under
    /// (what the toast libraries use: Windows shows them under PowerShell).
    pub const POWERSHELL_APP_ID: &str =
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

    /// Hand `click` to the handler, or keep it until there is one.
    pub fn deliver(click: NotificationClick) {
        let Ok(mut clicks) = CLICKS.lock() else {
            return;
        };
        match &clicks.0 {
            Some(handler) => handler(click),
            None => clicks.1.push(click),
        }
    }

    /// What the toast's `launch` argument carries.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Launch {
        id: String,
        payload: Option<String>,
    }

    pub fn encode_launch(identifier: &str, payload: Option<&str>) -> String {
        serde_json::to_string(&Launch {
            id: identifier.to_string(),
            payload: payload.map(str::to_string),
        })
        .unwrap_or_default()
    }

    pub fn decode_launch(arguments: &str) -> Option<NotificationClick> {
        let launch: Launch = serde_json::from_str(arguments).ok()?;
        Some(NotificationClick {
            identifier: launch.id,
            payload: launch.payload,
        })
    }

    fn xml_escape(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&apos;"),
                c => out.push(c),
            }
        }
        out
    }

    /// The `ToastGeneric` toast: title, body, and the click's argument.
    pub fn toast_xml(title: &str, body: &str, launch: &str) -> String {
        format!(
            "<toast launch=\"{}\" activationType=\"foreground\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
            xml_escape(launch),
            xml_escape(title),
            xml_escape(body)
        )
    }

    /// Post the toast under `app_id`. `in_process_clicks`: also take clicks
    /// through the toast's own event (a build without an activator).
    pub fn show(app_id: &str, xml: &str, in_process_clicks: bool) -> windows::core::Result<()> {
        // SAFETY: COM initialisation of this thread; a thread already
        // initialised answers RPC_E_CHANGED_MODE, which changes nothing here
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        if in_process_clicks {
            toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
                |_, arguments: Ref<IInspectable>| {
                    if let Some(click) = arguments
                        .ok()
                        .ok()
                        .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
                        .and_then(|args| args.Arguments().ok())
                        .and_then(|args| decode_launch(&args.to_string_lossy()))
                    {
                        deliver(click);
                    }
                    Ok(())
                },
            ))?;
        }
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?.Show(&toast)
    }

    /// The COM object a click activates.
    #[windows::core::implement(INotificationActivationCallback)]
    struct Activator;

    impl INotificationActivationCallback_Impl for Activator_Impl {
        fn Activate(
            &self,
            _app_user_model_id: &PCWSTR,
            invoked_arguments: &PCWSTR,
            _data: *const NOTIFICATION_USER_INPUT_DATA,
            _count: u32,
        ) -> windows::core::Result<()> {
            if invoked_arguments.is_null() {
                return Ok(());
            }
            // SAFETY: COM hands a NUL-terminated string that lives for the call
            let arguments = unsafe { invoked_arguments.to_string() }.unwrap_or_default();
            match decode_launch(&arguments) {
                Some(click) => deliver(click),
                None => {
                    tracing::debug!(%arguments, "toast activated without a Corvene argument")
                }
            }
            Ok(())
        }
    }

    /// Makes [`Activator`]s for COM.
    #[windows::core::implement(IClassFactory)]
    struct ActivatorFactory;

    impl IClassFactory_Impl for ActivatorFactory_Impl {
        fn CreateInstance(
            &self,
            outer: Ref<IUnknown>,
            iid: *const GUID,
            object: *mut *mut c_void,
        ) -> windows::core::Result<()> {
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let activator: INotificationActivationCallback = Activator.into();
            // SAFETY: `iid` and `object` are the out-parameters COM passed
            unsafe { activator.query(iid, object) }.ok()
        }

        fn LockServer(&self, _lock: BOOL) -> windows::core::Result<()> {
            Ok(())
        }
    }

    /// Register the activator's class object with COM on a thread of its
    /// own (a multithreaded apartment: activations arrive on COM's threads)
    /// and keep it registered for the life of the process.
    pub fn serve() {
        let spawned = std::thread::Builder::new()
            .name("toast-activator".into())
            .spawn(|| {
                // SAFETY: a fresh thread, initialised once
                if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
                    tracing::warn!("could not initialise COM for the toast activator");
                    return;
                }
                let factory: IClassFactory = ActivatorFactory.into();
                // SAFETY: `factory` outlives the registration (this thread never ends)
                let registered = unsafe {
                    CoRegisterClassObject(
                        &crate::windows::TOAST_ACTIVATOR_CLSID,
                        &factory,
                        CLSCTX_LOCAL_SERVER,
                        REGCLS_MULTIPLEUSE,
                    )
                };
                match registered {
                    Ok(_) => loop {
                        std::thread::park();
                    },
                    Err(err) => tracing::warn!(%err, "could not register the toast activator"),
                }
            });
        if let Err(err) = spawned {
            tracing::warn!(%err, "could not start the toast activator thread");
        }
    }
}

/// A toast under Corvene's AppUserModelID (an installed Corvene) or
/// PowerShell's (a build that is not installed), carrying the identifier and
/// payload for a click.
#[cfg(windows)]
pub fn show(
    identifier: &str,
    title: &str,
    body: &str,
    payload: Option<&str>,
    done: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) {
    let installed = crate::windows::installed();
    let app_id = if installed {
        crate::windows::APP_USER_MODEL_ID
    } else {
        win::POWERSHELL_APP_ID
    };
    let xml = win::toast_xml(title, body, &win::encode_launch(identifier, payload));
    let spawned = std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || {
            done(
                win::show(app_id, &xml, !installed)
                    .map_err(|err| NotificationError::Post(err.to_string())),
            )
        });
    if let Err(err) = spawned {
        tracing::warn!(%err, "could not start the notification thread");
    }
}

/// Clicks come from the toast activator (see [`serve_activator`]) or, for a
/// build that is not installed, from the toast's own event; the ones that
/// arrived before `handler` was installed are delivered now.
#[cfg(windows)]
pub fn install_click_handler(handler: impl Fn(NotificationClick) + Send + Sync + 'static) {
    let Ok(mut clicks) = win::CLICKS.lock() else {
        return;
    };
    if clicks.0.is_some() {
        return;
    }
    for click in clicks.1.drain(..) {
        handler(click);
    }
    clicks.0 = Some(Box::new(handler));
}

/// Windows: serve the toast activator COM object (the installer registers
/// this program as its local server) for the life of the process; a click
/// on a toast of an exited Corvene starts this program with `-Embedding`
/// and COM calls the activator here.
#[cfg(windows)]
pub fn serve_activator() {
    win::serve();
}

/// GHD `getNotificationSettingsUrl`: System Settings › Notifications for this app.
pub fn settings_url(bundle_id: &str) -> String {
    #[cfg(target_os = "android")]
    {
        let _ = bundle_id;
        crate::android::NOTIFICATION_SETTINGS_URL.to_string()
    }
    #[cfg(windows)]
    {
        let _ = bundle_id;
        "ms-settings:notifications".to_string()
    }
    #[cfg(not(any(target_os = "android", windows)))]
    format!("x-apple.systempreferences:com.apple.preference.notifications?id={bundle_id}")
}

// Android: a notification channel behind the activity's bridge. Posting
// needs the `POST_NOTIFICATIONS` permission from Android 13 on, asked for
// like macOS's authorization; a tap reopens the activity, which reports the
// click with the identifier and payload the notification carried, also for
// notifications of an earlier session.

#[cfg(target_os = "android")]
static ANDROID_CLICK_HANDLER: std::sync::OnceLock<Box<dyn Fn(NotificationClick) + Send + Sync>> =
    std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub fn permission() -> NotificationPermission {
    match crate::android::bridge().map(|bridge| bridge.notifications_allowed()) {
        Some(Some(true)) => NotificationPermission::Granted,
        Some(Some(false)) => NotificationPermission::Denied,
        Some(None) => NotificationPermission::Default,
        None => NotificationPermission::Unsupported,
    }
}

#[cfg(target_os = "android")]
pub fn request_permission() {
    if let Some(bridge) = crate::android::bridge() {
        bridge.request_notification_permission();
    }
}

#[cfg(target_os = "android")]
pub fn show(
    identifier: &str,
    title: &str,
    body: &str,
    payload: Option<&str>,
    done: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) {
    let Some(bridge) = crate::android::bridge() else {
        done(Err(NotificationError::Unsupported));
        return;
    };
    if bridge.notifications_allowed() == Some(false) {
        done(Err(NotificationError::NotGranted(None)));
        return;
    }
    bridge.show_notification(identifier, title, body, payload.unwrap_or_default());
    done(Ok(()));
}

#[cfg(target_os = "android")]
pub fn install_click_handler(handler: impl Fn(NotificationClick) + Send + Sync + 'static) {
    let _ = ANDROID_CLICK_HANDLER.set(Box::new(handler));
}

/// The activity was opened from a notification.
#[cfg(target_os = "android")]
pub fn clicked(click: NotificationClick) {
    if let Some(handler) = ANDROID_CLICK_HANDLER.get() {
        handler(click);
    }
}

/// A stand-in `org.freedesktop.Notifications` server on the session bus
/// (CI runs the tests under `dbus-run-session`): it records the posted
/// notification and "clicks" it. Skipped without a session bus or when a
/// real notification server owns the name.
#[cfg(all(test, not(any(target_os = "macos", target_os = "android", windows))))]
mod linux_tests {
    use std::collections::HashMap;
    use std::sync::mpsc;
    use std::time::Duration;

    use zbus::zvariant::OwnedValue;

    const PATH: &str = "/org/freedesktop/Notifications";
    const INTERFACE: &str = "org.freedesktop.Notifications";

    struct Server {
        posted: mpsc::Sender<(String, String, String, Vec<String>, u32)>,
    }

    #[zbus::interface(name = "org.freedesktop.Notifications")]
    impl Server {
        #[allow(clippy::too_many_arguments)]
        fn notify(
            &self,
            app_name: String,
            _replaces_id: u32,
            _app_icon: String,
            summary: String,
            body: String,
            actions: Vec<String>,
            _hints: HashMap<String, OwnedValue>,
            _expire_timeout: i32,
        ) -> u32 {
            let _ = self.posted.send((app_name, summary, body, actions, 7));
            7
        }

        fn get_capabilities(&self) -> Vec<String> {
            vec!["actions".into(), "body".into()]
        }

        fn get_server_information(&self) -> (String, String, String, String) {
            (
                "stand-in".into(),
                "corvene".into(),
                "1".into(),
                "1.2".into(),
            )
        }

        fn close_notification(&self, _id: u32) {}
    }

    #[test]
    fn posts_and_delivers_clicks() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            return;
        }
        let (posted_tx, posted_rx) = mpsc::channel();
        let Ok(connection) = zbus::blocking::connection::Builder::session()
            .and_then(|b| b.name(INTERFACE))
            .and_then(|b| b.serve_at(PATH, Server { posted: posted_tx }))
            .and_then(|b| b.build())
        else {
            return; // a real server has the name
        };
        let (click_tx, click_rx) = mpsc::channel();
        super::install_click_handler(move |click| {
            let _ = click_tx.send(click);
        });
        let (done_tx, done_rx) = mpsc::channel();
        super::show(
            "n-1",
            "Checks failed",
            "3 checks failed",
            Some("{}"),
            move |r| {
                let _ = done_tx.send(r.is_ok());
            },
        );
        let (app, summary, body, actions, id) =
            posted_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(app, "Corvene");
        assert_eq!(summary, "Checks failed");
        assert_eq!(body, "3 checks failed");
        assert_eq!(actions, ["default", "default"]);
        assert!(done_rx.recv_timeout(Duration::from_secs(10)).unwrap());
        // the waiting thread subscribes right after Notify returns
        std::thread::sleep(Duration::from_millis(500));
        connection
            .emit_signal(
                None::<()>,
                PATH,
                INTERFACE,
                "ActionInvoked",
                &(id, "default"),
            )
            .unwrap();
        let click = click_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(click.identifier, "n-1");
        assert_eq!(click.payload.as_deref(), Some("{}"));
        connection
            .emit_signal(
                None::<()>,
                PATH,
                INTERFACE,
                "NotificationClosed",
                &(id, 2u32),
            )
            .unwrap();
    }
}
