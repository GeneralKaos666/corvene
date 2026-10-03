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
//! Corvene has no button group: each dialog lists its footer
//! `corvene_ui::dialog::DialogButton`s in the macOS order (Cancel, then OK),
//! marks its default button `primary` itself (a destructive dialog marks
//! Cancel, e.g. `dialogs/discard_changes.rs`) and gives every button its own
//! `on_click`; `corvene_ui::dialog::ok_cancel_order` puts the list in the
//! platform's order. Buttons have no accessible description.
//!
//! So the ok-cancel case is ported twice:
//!
//! - [`renders_ok_cancel_buttons_in_platform_order_with_non_destructive_defaults`]
//!   checks the order through `ok_cancel_order`,
//! - [`renders_ok_cancel_buttons_in_platform_order_with_non_destructive_defaults_in_a_group`]
//!   checks every assertion of the case against [`ok_cancel_button_group`],
//!   a stand-in, like the destructive case. Replace the stand-in with the
//!   Corvene function once there is one and remove the `#[ignore]`s.
//!
//! The buttons' `type` is [`ButtonType`] (`submit` is the default button)
//! and a click is the [`GroupEvent`]s it sends. The CSS class assertions
//! (`.button-group.custom-buttons`) only check that the group is rendered,
//! which a returned group is; `.destructive` is the group's `destructive`.
//!
//! The other cases of the file render wrappers (`DialogContent`,
//! `DialogFooter`) and are skipped in `tools/ghd-tests/skips/ui1.tsv`.

use corvene_ui::dialog::ok_cancel_order;

/// A group button's `type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real OkCancelButtonGroup content
enum ButtonType {
    /// `type="submit"`: the default button, taken when the form is submitted
    /// with the keyboard (Corvene: `DialogButton::primary`).
    Submit,
    /// `type="reset"`
    Reset,
    /// `type="button"`
    Button,
}

/// What a click on a group button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real OkCancelButtonGroup content
enum GroupEvent {
    /// `onOkButtonClick`
    OkButtonClick,
    /// `onCancelButtonClick`
    CancelButtonClick,
    /// The dialog's form gets `submit` (its affirmative action).
    Submit,
    /// The dialog's form gets `reset` (it is dismissed).
    Reset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GroupButton {
    label: String,
    button_type: ButtonType,
    /// `aria-describedby`
    aria_described_by: Option<String>,
}

/// GitHub Desktop's `IOkCancelButtonGroupProps` (the click callbacks are
/// the [`GroupEvent`]s).
#[allow(dead_code)] // read by the real OkCancelButtonGroup content
struct OkCancelButtonGroupProps<'a> {
    destructive: bool,
    ok_button_text: &'a str,
    cancel_button_text: &'a str,
    ok_button_aria_described_by: Option<&'a str>,
    /// The text of each child.
    children: Vec<&'a str>,
}

/// What GitHub Desktop's `OkCancelButtonGroup` renders.
#[derive(Clone, Debug, PartialEq, Eq)]
struct OkCancelButtonGroupContent {
    /// The `destructive` class.
    destructive: bool,
    /// In the order they are drawn.
    buttons: Vec<GroupButton>,
    /// The children's text, after the buttons.
    children: Vec<String>,
}

impl OkCancelButtonGroupContent {
    /// `screen.getByRole('button', { name: label })`: exactly one button.
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

    /// `fireEvent.click` on the button labelled `label`: what it sends.
    fn click(&self, _label: &str) -> Vec<GroupEvent> {
        unimplemented!("Corvene has no OkCancelButtonGroup: each dialog wires its DialogButtons")
    }
}

/// Stand-in for GitHub Desktop's `OkCancelButtonGroup`
/// (`ui/dialog/ok-cancel-button-group.tsx`).
fn ok_cancel_button_group(_props: OkCancelButtonGroupProps<'_>) -> OkCancelButtonGroupContent {
    unimplemented!(
        "Corvene has no OkCancelButtonGroup: each dialog builds its DialogButtons and marks \
         the default one primary itself"
    )
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
#[ignore = "ghd: missing: no OkCancelButtonGroup (ui/dialog/ok-cancel-button-group.tsx); each Corvene dialog builds its DialogButtons and sets primary itself, buttons have no aria description and the footer takes no children"]
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
#[ignore = "ghd: missing: no OkCancelButtonGroup (ui/dialog/ok-cancel-button-group.tsx); each destructive Corvene dialog marks its Cancel DialogButton primary itself (e.g. dialogs/discard_changes.rs)"]
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
