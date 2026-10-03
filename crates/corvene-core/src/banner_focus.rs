//! GHD `Banner`'s focus and dismissal (`ui/banners/banner.tsx`
//! `componentDidMount`, `focusOnFirstSuitableElement`, `onFocusIn`,
//! `onFocusOut`): 200 ms after a banner appears focus moves to its first
//! link (or button); its dismissal timeout (`Banner::timeout`) starts only
//! when focus leaves the banner, and focus coming back cancels it. A banner
//! that is not dismissable, or has no timeout, stays until it is replaced
//! or closed.
//!
//! The clock is passed in (`now`), so the timers are driven by the caller:
//! `corvene_ui::banner::BannerView` runs them on GPUI timers.

use std::time::{Duration, Instant};

/// `componentDidMount`'s focus delay.
pub const BANNER_FOCUS_DELAY: Duration = Duration::from_millis(200);

/// What a due timer asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerFocusEvent {
    /// `focusOnFirstSuitableElement`: focus the banner's first link, else
    /// its first button.
    FocusFirstElement,
    /// `onDismissed`
    Dismiss,
}

/// One shown banner's focus and dismissal timers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BannerFocus {
    timeout: Option<Duration>,
    dismissable: bool,
    /// `focusTimeoutId`
    focus_at: Option<Instant>,
    /// `dismissalTimeoutId`
    dismiss_at: Option<Instant>,
}

impl BannerFocus {
    /// `componentDidMount` at `now` for a banner with `timeout` (GHD's
    /// `timeout` prop) that is `dismissable`.
    pub fn mount(now: Instant, timeout: Option<Duration>, dismissable: bool) -> Self {
        Self {
            timeout,
            dismissable,
            focus_at: Some(now + BANNER_FOCUS_DELAY),
            dismiss_at: None,
        }
    }

    /// `onFocusIn`: focus moved into the banner; a pending dismissal is
    /// cancelled.
    pub fn focus_in(&mut self) {
        self.dismiss_at = None;
    }

    /// `onFocusOut` at `now`; `related_target_inside`: focus moved to
    /// another element of the banner (nothing happens then).
    pub fn focus_out(&mut self, now: Instant, related_target_inside: bool) {
        if related_target_inside || !self.dismissable {
            return;
        }
        if let Some(timeout) = self.timeout {
            let at = now + timeout;
            self.dismiss_at = Some(self.dismiss_at.map_or(at, |pending| pending.min(at)));
        }
    }

    /// The timers due at `now`, in the order they fire; each fires once.
    pub fn advance(&mut self, now: Instant) -> Vec<BannerFocusEvent> {
        let mut due: Vec<(Instant, BannerFocusEvent)> = Vec::new();
        if let Some(at) = self.focus_at.filter(|at| *at <= now) {
            self.focus_at = None;
            due.push((at, BannerFocusEvent::FocusFirstElement));
        }
        if let Some(at) = self.dismiss_at.filter(|at| *at <= now) {
            self.dismiss_at = None;
            due.push((at, BannerFocusEvent::Dismiss));
        }
        due.sort_by_key(|(at, _)| *at);
        due.into_iter().map(|(_, event)| event).collect()
    }

    /// When the next timer is due, `None` with none pending.
    pub fn next_deadline(&self) -> Option<Instant> {
        match (self.focus_at, self.dismiss_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn focuses_after_200_ms_and_dismisses_only_after_focus_out() {
        let t0 = Instant::now();
        let mut banner = BannerFocus::mount(t0, Some(500 * MS), true);
        assert!(banner.advance(t0 + 199 * MS).is_empty());
        assert_eq!(
            banner.advance(t0 + 200 * MS),
            [BannerFocusEvent::FocusFirstElement]
        );
        // no timeout while focus stays
        assert!(banner.advance(t0 + 10_000 * MS).is_empty());
        assert_eq!(banner.next_deadline(), None);
        banner.focus_out(t0 + 10_000 * MS, false);
        banner.focus_in();
        assert!(banner.advance(t0 + 20_000 * MS).is_empty());
        banner.focus_out(t0 + 20_000 * MS, false);
        assert_eq!(banner.next_deadline(), Some(t0 + 20_500 * MS));
        assert_eq!(
            banner.advance(t0 + 20_500 * MS),
            [BannerFocusEvent::Dismiss]
        );
    }

    #[test]
    fn undismissable_and_untimed_banners_stay() {
        let t0 = Instant::now();
        let mut banner = BannerFocus::mount(t0, Some(500 * MS), false);
        banner.focus_out(t0, false);
        assert_eq!(banner.next_deadline(), Some(t0 + BANNER_FOCUS_DELAY));
        let mut untimed = BannerFocus::mount(t0, None, true);
        untimed.advance(t0 + BANNER_FOCUS_DELAY);
        untimed.focus_out(t0, false);
        assert_eq!(untimed.next_deadline(), None);
    }
}
