//! Port of GitHub Desktop's `app/test/unit/ui/dialog-action-wrappers-test.tsx`.
//!
//! GitHub Desktop's `OkCancelButtonGroup` (`ui/dialog/ok-cancel-button-group.tsx`)
//! renders a dialog's OK and Cancel buttons in the platform's order (Cancel
//! before OK on macOS, OK before Cancel elsewhere) and picks the default
//! button: OK is `type="submit"` and Cancel `type="reset"`, unless the group
//! is `destructive`, when OK is a plain `type="button"` and Cancel the
//! `submit` button, so that submitting the form with the keyboard takes the
//! safe choice. Whichever button is clicked, OK calls `onOkButtonClick` and
//! sends the dialog's form `submit` (its affirmative action), Cancel calls
//! `onCancelButtonClick` and sends `reset` (dismissal). Its children follow
//! the buttons; `okButtonAriaDescribedBy` describes the OK button.
//!
//! Corvene's counterpart is `corvene_ui::dialog::ok_cancel_button_group(props)`,
//! the content `corvene_ui::dialog::OkCancelButtonGroup` turns into a
//! dialog's footer `DialogButton`s (the `submit` button is the `primary`
//! one; dialogs list Cancel, then OK, and `ok_cancel_order` puts the footer
//! in the platform's order). The ok-cancel case is ported twice:
//!
//! - [`renders_ok_cancel_buttons_in_platform_order_with_non_destructive_defaults`]
//!   checks the order through `ok_cancel_order`,
//! - [`renders_ok_cancel_buttons_in_platform_order_with_non_destructive_defaults_in_a_group`]
//!   checks every assertion of the case against `ok_cancel_button_group`,
//!   like the destructive case.
//!
//! The buttons' `type` is `ButtonType` (`submit` is the default button)
//! and a click is the `GroupEvent`s it sends
//! (`OkCancelButtonGroupContent::click`). The CSS class assertions
//! (`.button-group.custom-buttons`) only check that the group is rendered,
//! which a returned group is; `.destructive` is the group's `destructive`.
//!
//! The other cases of the file render wrappers (`DialogContent`,
//! `DialogFooter`) and are skipped in `tools/ghd-tests/skips/ui1.tsv`.

use corvene_ui::dialog::{
    ButtonType, GroupButton, GroupEvent, OkCancelButtonGroupContent, OkCancelButtonGroupProps,
    ok_cancel_button_group, ok_cancel_order,
};

/// `screen.getByRole('button', { name: label })` on a rendered group.
trait GetButtonByName {
    /// Exactly one button named `label`.
    fn button(&self, label: &str) -> &GroupButton;
}

impl GetButtonByName for OkCancelButtonGroupContent {
    fn button(&self, label: &str) -> &GroupButton {
        let matches: Vec<&GroupButton> = self.buttons.iter().filter(|b| b.label == label).collect();
        assert_eq!(
            matches.len(),
            1,
            "expected exactly one button named {label:?} in {:?}",
            self.buttons
        );
        matches[0]
    }
}

// GHD: unit/ui/dialog-action-wrappers-test.tsx › dialog action wrappers › renders ok-cancel buttons in platform order with non-destructive defaults
#[test]
fn renders_ok_cancel_buttons_in_platform_order_with_non_destructive_defaults() {
    // `okButtonText="Apply"`, `cancelButtonText="Dismiss"`, listed Cancel
    // then OK as Corvene's dialogs do
    let names = ok_cancel_order(vec!["Dismiss", "Apply"]);

    assert_eq!(
        names,
        if cfg!(target_os = "macos") {
            ["Dismiss", "Apply"]
        } else {
            ["Apply", "Dismiss"]
        }
    );
}

// GHD: unit/ui/dialog-action-wrappers-test.tsx › dialog action wrappers › renders ok-cancel buttons in platform order with non-destructive defaults
#[test]
fn renders_ok_cancel_buttons_in_platform_order_with_non_destructive_defaults_in_a_group() {
    let group = ok_cancel_button_group(OkCancelButtonGroupProps {
        destructive: false,
        ok_button_text: "Apply",
        cancel_button_text: "Dismiss",
        ok_button_aria_described_by: Some("apply-help"),
        children: vec!["Extra action"],
    });

    // `screen.getAllByRole('button')` (the `<span>` child is no button)
    let names: Vec<&str> = group.buttons.iter().map(|b| b.label.as_str()).collect();

    assert_eq!(
        names,
        if cfg!(target_os = "macos") {
            ["Dismiss", "Apply"]
        } else {
            ["Apply", "Dismiss"]
        }
    );
    assert_eq!(group.button("Apply").button_type, ButtonType::Submit);
    assert_eq!(group.button("Dismiss").button_type, ButtonType::Reset);
    assert_eq!(
        group.button("Apply").aria_described_by.as_deref(),
        Some("apply-help")
    );
    // `screen.getByText('Extra action')`: exactly one element
    assert_eq!(
        group
            .children
            .iter()
            .filter(|child| *child == "Extra action")
            .count(),
        1
    );
}

// GHD: unit/ui/dialog-action-wrappers-test.tsx › dialog action wrappers › dispatches submit and reset events for destructive button groups
#[test]
fn dispatches_submit_and_reset_events_for_destructive_button_groups() {
    let group = ok_cancel_button_group(OkCancelButtonGroupProps {
        destructive: true,
        ok_button_text: "Delete",
        cancel_button_text: "Keep",
        ok_button_aria_described_by: None,
        children: Vec::new(),
    });

    // `.button-group.destructive` is not null
    assert!(group.destructive);
    assert_eq!(group.button("Delete").button_type, ButtonType::Button);
    assert_eq!(group.button("Keep").button_type, ButtonType::Submit);

    let mut events = group.click("Delete");
    events.extend(group.click("Keep"));
    let count = |event: GroupEvent| events.iter().filter(|e| **e == event).count();

    // okClicks `['ok']`, cancelClicks `['cancel']`, submitted `['submit']`,
    // reset `['reset']`
    assert_eq!(count(GroupEvent::OkButtonClick), 1);
    assert_eq!(count(GroupEvent::CancelButtonClick), 1);
    assert_eq!(count(GroupEvent::Submit), 1);
    assert_eq!(count(GroupEvent::Reset), 1);
}
