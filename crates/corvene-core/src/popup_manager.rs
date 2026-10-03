//! The stack of open popups (GHD `app/src/lib/popup-manager.ts`
//! `PopupManager`), behind `AppState::popups`.
//!
//! As in GitHub Desktop:
//!
//! - every popup added gets the next id, from 1, counting on after
//!   removals (GHD `popup.id`; here [`StackedPopup::id`]);
//! - there is at most one popup of each type ([`PopupType`], the `Popup`
//!   variant), except errors, which may repeat;
//! - error popups stay on top: another popup goes in below them, and the
//!   current popup is the last one;
//! - past the limit (50 by default) the oldest popup is dropped.
//!
//! Corvene's error popups are `Popup::Error` and `Popup::IndexLockExists`
//! (flag `265-remove-stale-index-lock`, an error with a button to remove
//! the lock file: what GHD shows as an `Error` popup), see
//! [`Popup::is_error`]. GHD reports a full stack with
//! `sendNonFatalException('TooManyPopups')`; Corvene sends nothing (no
//! telemetry) and logs it.

use tracing::{error, warn};

use crate::state::{ErrorMessage, Popup};

/// GHD `defaultPopupStackLimit`: a user should only deal with a couple of
/// popups at a time, so hitting this limit means something is wrong.
pub const DEFAULT_POPUP_STACK_LIMIT: usize = 50;

/// GHD `PopupType`: which kind of popup a `Popup` is (its variant).
pub type PopupType = std::mem::Discriminant<Popup>;

/// A popup with the id the stack gave it (GHD's `Popup` with `id?`):
/// `None` for one never added (a popup literal), or one refused as a
/// duplicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackedPopup {
    pub id: Option<u64>,
    pub popup: Popup,
}

impl StackedPopup {
    /// `popup.type`
    pub fn popup_type(&self) -> PopupType {
        self.popup.popup_type()
    }
}

impl From<Popup> for StackedPopup {
    /// A popup literal (`{ type: PopupType.About }`): no id yet.
    fn from(popup: Popup) -> Self {
        Self { id: None, popup }
    }
}

/// What an error popup shows (GHD `addErrorPopup(error: Error)`): a title
/// and the error. A bare message gets GHD's `AppError` title, "Error".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppError {
    pub title: String,
    pub message: ErrorMessage,
}

impl AppError {
    pub fn new(title: impl Into<String>, message: impl Into<ErrorMessage>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self::new("Error", message)
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::new("Error", message)
    }
}

impl From<AppError> for Popup {
    fn from(error: AppError) -> Self {
        let ErrorMessage { text, git } = error.message;
        Popup::Error {
            title: error.title,
            message: text,
            git,
        }
    }
}

/// GHD `PopupManager`: the stack of currently open popups.
#[derive(Clone, Debug)]
pub struct PopupManager {
    popup_stack: Vec<StackedPopup>,
    popup_counter: u64,
    popup_limit: usize,
}

impl Default for PopupManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PopupManager {
    /// `new PopupManager()`
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_POPUP_STACK_LIMIT)
    }

    /// `new PopupManager(popupLimit)`
    pub fn with_limit(popup_limit: usize) -> Self {
        Self {
            popup_stack: Vec::new(),
            popup_counter: 0,
            popup_limit,
        }
    }

    /// `currentPopup`: the last popup of the stack (the last error popup
    /// when there are any, else the last other one).
    pub fn current_popup(&self) -> Option<&StackedPopup> {
        self.popup_stack.last()
    }

    /// `allPopups`, oldest first; error popups last.
    pub fn all_popups(&self) -> &[StackedPopup] {
        &self.popup_stack
    }

    /// `isAPopupOpen`
    pub fn is_a_popup_open(&self) -> bool {
        self.current_popup().is_some()
    }

    /// `getPopupsOfType(popupType)`
    pub fn get_popups_of_type(&self, popup_type: PopupType) -> Vec<StackedPopup> {
        self.popup_stack
            .iter()
            .filter(|p| p.popup_type() == popup_type)
            .cloned()
            .collect()
    }

    /// `areTherePopupsOfType(popupType)`
    pub fn are_there_popups_of_type(&self, popup_type: PopupType) -> bool {
        self.popup_stack
            .iter()
            .any(|p| p.popup_type() == popup_type)
    }

    /// `addPopup(popup)`: adds the popup with a new id and returns it. A
    /// popup of a type already on the stack is not added (it is returned
    /// as given); errors go through [`Self::add_error_popup`].
    pub fn add_popup(&mut self, popup_to_add: impl Into<StackedPopup>) -> StackedPopup {
        let popup_to_add = popup_to_add.into();
        if popup_to_add.popup.is_error() {
            return self.push_error_popup(popup_to_add);
        }

        // `{ id: ++this.popupCounter, ...popupToAdd }`: the counter moves
        // on even when the popup is refused below
        self.popup_counter += 1;
        let popup = StackedPopup {
            id: popup_to_add.id.or(Some(self.popup_counter)),
            popup: popup_to_add.popup.clone(),
        };

        if self.are_there_popups_of_type(popup_to_add.popup_type()) {
            warn!(
                popup = ?popup_to_add.popup_type(),
                "Attempted to add a popup of already existing type"
            );
            return popup_to_add;
        }

        self.insert_before_error_popups(popup.clone());
        self.check_stack_length();
        popup
    }

    /// `insertBeforeErrorPopups`: a non-error popup goes in below any error
    /// popups.
    fn insert_before_error_popups(&mut self, popup: StackedPopup) {
        if !self.popup_stack.last().is_some_and(|p| p.popup.is_error()) {
            self.popup_stack.push(popup);
            return;
        }
        // `[...nonErrorPopups, popup, ...errorPopups]`
        let (errors, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.popup_stack)
            .into_iter()
            .partition(|p| p.popup.is_error());
        self.popup_stack = others;
        self.popup_stack.push(popup);
        self.popup_stack.extend(errors);
    }

    /// `addErrorPopup(error)`: adds an error popup with a new id; error
    /// popups may repeat.
    pub fn add_error_popup(&mut self, error: impl Into<AppError>) -> StackedPopup {
        self.push_error_popup(StackedPopup::from(Popup::from(error.into())))
    }

    fn push_error_popup(&mut self, error: StackedPopup) -> StackedPopup {
        self.popup_counter += 1;
        let popup = StackedPopup {
            id: Some(self.popup_counter),
            popup: error.popup,
        };
        self.popup_stack.push(popup.clone());
        self.check_stack_length();
        popup
    }

    /// `checkStackLength`: past the limit the oldest popup goes.
    fn check_stack_length(&mut self) {
        if self.popup_stack.len() > self.popup_limit {
            let oldest = self.popup_stack.remove(0);
            error!(
                limit = self.popup_limit,
                adding = ?self.current_popup().map(StackedPopup::popup_type),
                removed = ?oldest.popup,
                "Max number of popups reached; removing the oldest popup from the stack"
            );
        }
    }

    /// `updatePopup(popup)`: replaces the popup with the same id; a popup
    /// without an id or not on the stack is ignored.
    pub fn update_popup(&mut self, popup_to_update: StackedPopup) {
        let Some(id) = popup_to_update.id else {
            warn!("Attempted to update a popup without an id.");
            return;
        };
        match self.popup_stack.iter_mut().find(|p| p.id == Some(id)) {
            Some(slot) => *slot = popup_to_update,
            None => warn!("Attempted to update a popup not in the stack."),
        }
    }

    /// `removePopup(popup)`: removes the popup with the same id; a popup
    /// without an id removes nothing.
    pub fn remove_popup(&mut self, popup: impl Into<StackedPopup>) {
        let popup = popup.into();
        let Some(id) = popup.id else {
            warn!("Attempted to remove a popup without an id.");
            return;
        };
        self.popup_stack.retain(|p| p.id != Some(id));
    }

    /// `removePopupByType(popupType)`: removes every popup of that type.
    pub fn remove_popup_by_type(&mut self, popup_type: PopupType) {
        self.popup_stack.retain(|p| p.popup_type() != popup_type);
    }

    /// `removePopupById(popupId)`
    pub fn remove_popup_by_id(&mut self, popup_id: u64) {
        self.popup_stack.retain(|p| p.id != Some(popup_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn about() -> Popup {
        Popup::About {
            version: String::new(),
        }
    }

    #[test]
    fn a_popup_added_below_errors_keeps_the_errors_on_top() {
        let mut manager = PopupManager::new();
        manager.add_popup(about());
        manager.add_error_popup("first");
        manager.add_error_popup("second");
        let sign_in = manager.add_popup(Popup::SignIn { enterprise: false });

        let order: Vec<Option<u64>> = manager.all_popups().iter().map(|p| p.id).collect();
        assert_eq!(order, vec![Some(1), sign_in.id, Some(2), Some(3)]);
        assert_eq!(sign_in.id, Some(4));
        assert!(manager.current_popup().is_some_and(|p| p.popup.is_error()));
    }

    #[test]
    fn a_refused_duplicate_still_uses_up_an_id() {
        let mut manager = PopupManager::new();
        manager.add_popup(about());
        let refused = manager.add_popup(about());
        assert_eq!(refused.id, None);
        assert_eq!(manager.add_popup(Popup::Acknowledgements).id, Some(3));
    }

    #[test]
    fn a_bare_error_message_is_titled_error() {
        let mut manager = PopupManager::new();
        let popup = manager.add_error_popup("an error");
        assert_eq!(
            popup.popup,
            Popup::Error {
                title: "Error".into(),
                message: "an error".into(),
                git: None,
            }
        );
    }
}
