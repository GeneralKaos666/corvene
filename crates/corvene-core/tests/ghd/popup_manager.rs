//! Port of GitHub Desktop's `app/test/unit/popup-manager-test.ts`
//! (`lib/popup-manager.ts`).
//!
//! GitHub Desktop's `PopupManager` is the stack of open popups: one popup
//! per type except `Error`, which may repeat; error popups stay on top;
//! every added popup gets the next id (from 1); the oldest popup is dropped
//! past the limit (50 by default). Corvene has no such stack:
//! `AppState::popup` is a single `Option<Popup>` that
//! `Dispatcher::show_popup` replaces and `Dispatcher::close_popup` clears
//! (`crates/corvene-core/src/state.rs`, `dispatcher.rs`; `deviations.md`
//! mentions "Corvene shows one popup at a time" in passing, under Pull
//! request notification dialogs). [`PopupManager`] is a stand-in over
//! Corvene's `Popup`; a popup with its stack id (GitHub Desktop's
//! `popup.id`) is a [`StackedPopup`], and a popup's type (GitHub Desktop's
//! `PopupType`) is its enum variant ([`popup_type`]).
//!
//! GitHub Desktop's `PopupType.TermsAndConditions` has no Corvene popup
//! (About's Terms and Conditions link is omitted, `deviations.md` › About);
//! the cases use `Popup::Acknowledgements`, another popup without
//! arguments, in its place: the manager treats every non-error type alike.
//! `new Error(message)` is the message of a `Popup::Error`.

use std::mem::Discriminant;

use corvene_core::{Account, Popup};

/// GitHub Desktop's `PopupType`: which variant a popup is.
type PopupType = Discriminant<Popup>;

fn popup_type(popup: &Popup) -> PopupType {
    std::mem::discriminant(popup)
}

/// A popup as GitHub Desktop's `PopupManager` hands it out: with the id it
/// was given, or `None` for one never added (GitHub Desktop's `id?`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct StackedPopup {
    id: Option<u64>,
    popup: Popup,
}

impl StackedPopup {
    /// `popup.type`
    fn popup_type(&self) -> PopupType {
        popup_type(&self.popup)
    }
}

impl From<Popup> for StackedPopup {
    /// A popup literal (`{ type: PopupType.About }`): no id yet.
    fn from(popup: Popup) -> Self {
        Self { id: None, popup }
    }
}

/// Stand-in for GitHub Desktop's `PopupManager` (`lib/popup-manager.ts`).
/// Replace it with the Corvene type once there is one and remove the
/// `#[ignore]`s.
struct PopupManager;

const UNIMPLEMENTED: &str = "Corvene has no PopupManager (lib/popup-manager.ts)";

impl PopupManager {
    /// `new PopupManager()` (limit 50)
    fn new() -> Self {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `new PopupManager(popupLimit)`
    fn with_limit(_popup_limit: usize) -> Self {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `currentPopup`
    fn current_popup(&self) -> Option<StackedPopup> {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `isAPopupOpen`
    fn is_a_popup_open(&self) -> bool {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `getPopupsOfType(popupType)`
    fn get_popups_of_type(&self, _popup_type: PopupType) -> Vec<StackedPopup> {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `areTherePopupsOfType(popupType)`
    fn are_there_popups_of_type(&self, _popup_type: PopupType) -> bool {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `addPopup(popup)`
    fn add_popup(&mut self, _popup: impl Into<StackedPopup>) -> StackedPopup {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `addErrorPopup(new Error(message))`
    fn add_error_popup(&mut self, _message: &str) -> StackedPopup {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `updatePopup(popup)`
    fn update_popup(&mut self, _popup: StackedPopup) {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `removePopup(popup)`
    fn remove_popup(&mut self, _popup: impl Into<StackedPopup>) {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `removePopupByType(popupType)`
    fn remove_popup_by_type(&mut self, _popup_type: PopupType) {
        unimplemented!("{UNIMPLEMENTED}")
    }

    /// `removePopupById(popupId)`
    fn remove_popup_by_id(&mut self, _popup_id: u64) {
        unimplemented!("{UNIMPLEMENTED}")
    }
}

/// `{ type: PopupType.About }`
fn about() -> Popup {
    Popup::About {
        version: String::new(),
    }
}

/// `{ type: PopupType.SignIn }`
fn sign_in() -> Popup {
    Popup::SignIn { enterprise: false }
}

/// `{ type: PopupType.TermsAndConditions }` (see the module doc)
fn terms_and_conditions() -> Popup {
    Popup::Acknowledgements
}

/// `PopupType.Error`
fn error_type() -> PopupType {
    popup_type(&Popup::Error {
        title: String::new(),
        message: String::new(),
        git: None,
    })
}

/// `PopupType.CreateTutorialRepository`
fn create_tutorial_repository_type(account: &Account) -> PopupType {
    popup_type(&Popup::CreateTutorialRepository {
        account: account.clone(),
        progress: None,
    })
}

// GHD: unit/popup-manager-test.ts › PopupManager › currentPopup › returns null when no popups added
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn current_popup_returns_null_when_no_popups_added() {
    let popup_manager = PopupManager::new();
    assert!(popup_manager.current_popup().is_none());
}

// GHD: unit/popup-manager-test.ts › PopupManager › currentPopup › returns last added non-error popup
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn current_popup_returns_last_added_non_error_popup() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());

    let current_popup = popup_manager.current_popup();
    assert!(current_popup.is_some());
    assert_eq!(
        current_popup.map(|p| p.popup_type()),
        Some(popup_type(&sign_in()))
    );
}

// GHD: unit/popup-manager-test.ts › PopupManager › currentPopup › returns last added error popup
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn current_popup_returns_last_added_error_popup() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());
    popup_manager.add_error_popup("an error");
    popup_manager.add_popup(sign_in());

    let current_popup = popup_manager.current_popup();
    assert!(current_popup.is_some());
    assert_eq!(current_popup.map(|p| p.popup_type()), Some(error_type()));
}

// GHD: unit/popup-manager-test.ts › PopupManager › isAPopupOpen › returns false when no popups added
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn is_a_popup_open_returns_false_when_no_popups_added() {
    let popup_manager = PopupManager::new();
    assert!(!popup_manager.is_a_popup_open());
}

// GHD: unit/popup-manager-test.ts › PopupManager › isAPopupOpen › returns last added popup
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn is_a_popup_open_returns_last_added_popup() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());

    let is_a_popup_open = popup_manager.is_a_popup_open();
    assert!(is_a_popup_open);
}

// GHD: unit/popup-manager-test.ts › PopupManager › getPopupsOfType › returns popups of a given type
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn get_popups_of_type_returns_popups_of_a_given_type() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());

    let about_popups = popup_manager.get_popups_of_type(popup_type(&about()));
    assert_eq!(about_popups.len(), 1);
    assert_eq!(
        about_popups.first().map(StackedPopup::popup_type),
        Some(popup_type(&about()))
    );
}

// GHD: unit/popup-manager-test.ts › PopupManager › getPopupsOfType › returns empty array if none exist of given type
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn get_popups_of_type_returns_empty_array_if_none_exist_of_given_type() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());

    let sign_in_popups = popup_manager.get_popups_of_type(popup_type(&sign_in()));
    assert_eq!(sign_in_popups.len(), 0);
}

// GHD: unit/popup-manager-test.ts › PopupManager › areTherePopupsOfType › returns true if popup of type exists
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn are_there_popups_of_type_returns_true_if_popup_of_type_exists() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());

    assert!(popup_manager.are_there_popups_of_type(popup_type(&about())));
}

// GHD: unit/popup-manager-test.ts › PopupManager › areTherePopupsOfType › returns false if there are no popups of that type
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn are_there_popups_of_type_returns_false_if_there_are_no_popups_of_that_type() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());

    assert!(!popup_manager.are_there_popups_of_type(popup_type(&sign_in())));
}

// GHD: unit/popup-manager-test.ts › PopupManager › addPopup › adds a popup to the stack
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn add_popup_adds_a_popup_to_the_stack() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());

    let popups_of_type = popup_manager.get_popups_of_type(popup_type(&about()));
    let current_popup = popup_manager.current_popup();
    assert_eq!(popups_of_type.len(), 1);
    assert!(current_popup.is_some());
    assert_eq!(
        current_popup.map(|p| p.popup_type()),
        Some(popup_type(&about()))
    );
}

// GHD: unit/popup-manager-test.ts › PopupManager › addPopup › does not add multiple popups of the same kind to the stack
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn add_popup_does_not_add_multiple_popups_of_the_same_kind_to_the_stack() {
    let mut popup_manager = PopupManager::new();
    let popup = about();
    popup_manager.add_popup(popup.clone());
    popup_manager.add_popup(popup);

    let popups_of_type = popup_manager.get_popups_of_type(popup_type(&about()));
    assert_eq!(popups_of_type.len(), 1);
}

// GHD: unit/popup-manager-test.ts › PopupManager › addPopup › adds multiple popups of different types
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn add_popup_adds_multiple_popups_of_different_types() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());

    let about_popups = popup_manager.get_popups_of_type(popup_type(&about()));
    let sign_in_poups = popup_manager.get_popups_of_type(popup_type(&sign_in()));
    assert_eq!(about_popups.len(), 1);
    assert_eq!(sign_in_poups.len(), 1);

    assert_eq!(
        about_popups.first().map(StackedPopup::popup_type),
        Some(popup_type(&about()))
    );
    assert_eq!(
        sign_in_poups.first().map(StackedPopup::popup_type),
        Some(popup_type(&sign_in()))
    );
}

// GHD: unit/popup-manager-test.ts › PopupManager › addPopup › trims oldest popup when limit is reached
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn add_popup_trims_oldest_popup_when_limit_is_reached() {
    let mut popup_manager = PopupManager::with_limit(2);
    popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());
    popup_manager.add_popup(terms_and_conditions());

    let about_popups = popup_manager.get_popups_of_type(popup_type(&about()));
    let sign_in_poups = popup_manager.get_popups_of_type(popup_type(&sign_in()));
    let terms_and_conditions_poups =
        popup_manager.get_popups_of_type(popup_type(&terms_and_conditions()));
    assert_eq!(about_popups.len(), 0);
    assert_eq!(sign_in_poups.len(), 1);
    assert_eq!(terms_and_conditions_poups.len(), 1);

    assert_eq!(
        sign_in_poups.first().map(StackedPopup::popup_type),
        Some(popup_type(&sign_in()))
    );
    assert_eq!(
        terms_and_conditions_poups
            .first()
            .map(StackedPopup::popup_type),
        Some(popup_type(&terms_and_conditions()))
    );
}

// GHD: unit/popup-manager-test.ts › PopupManager › addErrorPopup › adds a popup of type error to the stack
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn add_error_popup_adds_a_popup_of_type_error_to_the_stack() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_error_popup("an error");

    let popups_of_type = popup_manager.get_popups_of_type(error_type());
    let current_popup = popup_manager.current_popup();
    assert_eq!(popups_of_type.len(), 1);
    assert!(current_popup.is_some());
    assert_eq!(current_popup.map(|p| p.popup_type()), Some(error_type()));
}

// GHD: unit/popup-manager-test.ts › PopupManager › addErrorPopup › adds multiple popups of type error to the stack
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn add_error_popup_adds_multiple_popups_of_type_error_to_the_stack() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_error_popup("an error");
    popup_manager.add_error_popup("an error");

    let popups_of_type = popup_manager.get_popups_of_type(error_type());
    assert_eq!(popups_of_type.len(), 2);
}

// GHD: unit/popup-manager-test.ts › PopupManager › addErrorPopup › trims oldest popup when limit is reached
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn add_error_popup_trims_oldest_popup_when_limit_is_reached() {
    let limit = 2;
    let mut popup_manager = PopupManager::with_limit(limit);
    popup_manager.add_error_popup("an error");
    popup_manager.add_error_popup("an error");
    popup_manager.add_error_popup("an error");
    popup_manager.add_error_popup("an error");

    let error_popups = popup_manager.get_popups_of_type(error_type());
    assert_eq!(error_popups.len(), limit);
}

// GHD: unit/popup-manager-test.ts › PopupManager › updatePopup › updates the given popup
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn update_popup_updates_the_given_popup() {
    // `new Account('test', '', 'deadbeef', [], '', 1, '', 'free')`: Corvene's
    // account keeps its token in the keychain, not in the model
    let mock_account = Account {
        endpoint: String::new(),
        id: 1,
        login: "test".to_string(),
        name: Some(String::new()),
        avatar_url: Some(String::new()),
        emails: Vec::new(),
        scopes: Vec::new(),
        plan: Some("free".to_string()),
        private_primary_email: false,
    };
    let popup_tutorial = Popup::CreateTutorialRepository {
        account: mock_account.clone(),
        progress: None,
    };

    let mut popup_manager = PopupManager::new();
    let tutorial_popup = popup_manager.add_popup(popup_tutorial);

    // Just so update spreader notation will work
    let Popup::CreateTutorialRepository { account, .. } = &tutorial_popup.popup else {
        return;
    };

    // `progress: { kind: 'generic', value: 5 }`: Corvene's progress is
    // (title, percent, detail) and has no kind
    let updated_popup = StackedPopup {
        id: tutorial_popup.id,
        popup: Popup::CreateTutorialRepository {
            account: account.clone(),
            progress: Some((String::new(), 5, None)),
        },
    };
    popup_manager.update_popup(updated_popup);

    let result = popup_manager.get_popups_of_type(create_tutorial_repository_type(&mock_account));
    assert_eq!(result.len(), 1);
    let Some(resulting_popup) = result.first() else {
        // Would fail first expect if not
        return;
    };

    assert_eq!(
        resulting_popup.popup_type(),
        create_tutorial_repository_type(&mock_account)
    );
    let Popup::CreateTutorialRepository { progress, .. } = &resulting_popup.popup else {
        return;
    };

    assert_ne!(progress, &None);
    // GitHub Desktop also checks `progress.kind === 'generic'`, which has no
    // Corvene field
    assert_eq!(progress.as_ref().map(|(_, value, _)| *value), Some(5));
}

// GHD: unit/popup-manager-test.ts › PopupManager › removePopup › deletes popup when give a popup with an id
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn remove_popup_deletes_popup_when_give_a_popup_with_an_id() {
    let mut popup_manager = PopupManager::new();
    let popup_about = popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());

    popup_manager.remove_popup(popup_about);

    let about_popups = popup_manager.get_popups_of_type(popup_type(&about()));
    assert_eq!(about_popups.len(), 0);

    let sign_in_popups = popup_manager.get_popups_of_type(popup_type(&sign_in()));
    assert_eq!(sign_in_popups.len(), 1);
}

// GHD: unit/popup-manager-test.ts › PopupManager › removePopup › does not remove popups by type
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn remove_popup_does_not_remove_popups_by_type() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());

    popup_manager.remove_popup(about());

    let about_popups = popup_manager.get_popups_of_type(popup_type(&about()));
    assert_eq!(about_popups.len(), 1);

    let sign_in_popups = popup_manager.get_popups_of_type(popup_type(&sign_in()));
    assert_eq!(sign_in_popups.len(), 1);
}

// GHD: unit/popup-manager-test.ts › PopupManager › removePopupByType › removes the popups of a given type
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn remove_popup_by_type_removes_the_popups_of_a_given_type() {
    let mut popup_manager = PopupManager::new();
    popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());

    popup_manager.remove_popup_by_type(popup_type(&about()));

    let about_popups = popup_manager.get_popups_of_type(popup_type(&about()));
    assert_eq!(about_popups.len(), 0);

    let sign_in_popups = popup_manager.get_popups_of_type(popup_type(&sign_in()));
    assert_eq!(sign_in_popups.len(), 1);
}

// GHD: unit/popup-manager-test.ts › PopupManager › removePopupById › removes the popup by its id
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn remove_popup_by_id_removes_the_popup_by_its_id() {
    let mut popup_manager = PopupManager::new();
    let popup_about = popup_manager.add_popup(about());
    popup_manager.add_popup(sign_in());

    assert_ne!(popup_about.id, None);
    let Some(id) = popup_about.id else {
        return;
    };

    popup_manager.remove_popup_by_id(id);

    let about_popups = popup_manager.get_popups_of_type(popup_type(&about()));
    assert_eq!(about_popups.len(), 0);

    let sign_in_popups = popup_manager.get_popups_of_type(popup_type(&sign_in()));
    assert_eq!(sign_in_popups.len(), 1);
}

// GHD: unit/popup-manager-test.ts › PopupManager › popup id increment › assigns id starting at 1 for first popup
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn assigns_id_starting_at_1_for_first_popup() {
    let mut popup_manager = PopupManager::new();
    let popup = popup_manager.add_popup(about());
    assert_eq!(popup.id, Some(1));
}

// GHD: unit/popup-manager-test.ts › PopupManager › popup id increment › increments ids sequentially for multiple popups
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn increments_ids_sequentially_for_multiple_popups() {
    let mut popup_manager = PopupManager::new();
    let popup1 = popup_manager.add_popup(about());
    let popup2 = popup_manager.add_popup(sign_in());
    let popup3 = popup_manager.add_popup(terms_and_conditions());

    assert_eq!(popup1.id, Some(1));
    assert_eq!(popup2.id, Some(2));
    assert_eq!(popup3.id, Some(3));
}

// GHD: unit/popup-manager-test.ts › PopupManager › popup id increment › increments ids for error popups
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn increments_ids_for_error_popups() {
    let mut popup_manager = PopupManager::new();
    let popup1 = popup_manager.add_popup(about());
    let error_popup = popup_manager.add_error_popup("test error");

    assert_eq!(popup1.id, Some(1));
    assert_eq!(error_popup.id, Some(2));
}

// GHD: unit/popup-manager-test.ts › PopupManager › popup id increment › continues incrementing after popups are removed
#[test]
#[ignore = "ghd: missing: no PopupManager (lib/popup-manager.ts); AppState::popup is one Option<Popup>"]
fn continues_incrementing_after_popups_are_removed() {
    let mut popup_manager = PopupManager::new();
    let popup1 = popup_manager.add_popup(about());
    popup_manager.remove_popup(popup1.clone());
    let popup2 = popup_manager.add_popup(sign_in());

    assert_eq!(popup1.id, Some(1));
    assert_eq!(popup2.id, Some(2));
}
