//! Port of GitHub Desktop's
//! `app/test/unit/ui/small-action-and-dialog-surfaces-test.tsx`.
//!
//! - `CICheckRunNoStepItem` (`ui/check-runs/ci-check-run-no-steps.tsx`):
//!   "There are no steps to display for this check.", a "View check
//!   details" button calling `onViewCheckExternally` and a decorative
//!   paper-stack image (`alt=""`). Corvene draws the same pane inline in
//!   the check run list of `crates/corvene-ui/src/ci_check_popover.rs` (a
//!   GPUI element; the button opens the check's URL with
//!   `Dispatcher::open_url`), so [`ci_check_run_no_step_item`] is a
//!   stand-in returning the pane's content.
//! - `CLIInstalled` (`ui/cli-installed/cli-installed.tsx`): the title
//!   ("Command Line Tool Installed" / "Command line tool installed"), "The
//!   command line tool has been installed at `InstalledCLIPath`." and an Ok
//!   button that dismisses the dialog. Corvene's dialog is
//!   `Popup::CLIInstalled { path }` drawn inline by `SimpleDialog`
//!   (`crates/corvene-ui/src/dialogs/simple.rs`), with the path
//!   `Dispatcher::install_cli` linked: `corvene_platform::cli::install_path()`,
//!   GitHub Desktop's `InstalledCLIPath` (`ui/lib/install-cli.ts`). The case
//!   is split in two tests: the path, ported to `install_path()` and
//!   ignored as the documented `corvene` rename (`.docs/deviations.md` ›
//!   Window / menus › Install Command Line Tool…; Linux's
//!   `~/.local/bin/corvene` is flag `414-linux-install-cli`'s Corvene
//!   addition), and the title and button, for which [`cli_installed`] is a
//!   stand-in.
//!
//! A click on a button is GitHub Desktop's callback prop; here the content
//! names the action the button runs, which is what `fireEvent.click` then
//! checks. Not ported (React DOM / test scaffolding): the `role="link"`
//! and the `img` element as such, and the `fs-admin` / `ipcRenderer.send`
//! mocks `CLIInstalled` needs to load.

use std::path::Path;

use corvene_platform::cli::install_path;

/// What a button of `CICheckRunNoStepItem` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real CICheckRunNoStepItem content
enum CheckRunNoStepAction {
    /// `onViewCheckExternally`
    ViewCheckExternally,
}

/// What GitHub Desktop's `CICheckRunNoStepItem` shows.
#[allow(dead_code)] // filled by the real CICheckRunNoStepItem content
struct CheckRunNoStepContent {
    /// The paragraph's text (the button's label excluded).
    text: String,
    /// The button: its label and what it does.
    button: (String, CheckRunNoStepAction),
    /// The paper-stack image's alternative text (`Some("")`: decorative),
    /// `None` when there is no image.
    image_alt: Option<String>,
}

/// Stand-in for GitHub Desktop's `CICheckRunNoStepItem`
/// (`ui/check-runs/ci-check-run-no-steps.tsx`). Replace it with the
/// Corvene function once there is one and remove the `#[ignore]`.
fn ci_check_run_no_step_item() -> CheckRunNoStepContent {
    unimplemented!(
        "Corvene has no CICheckRunNoStepItem content: ci_check_popover.rs draws the no-steps pane inline (GPUI)"
    )
}

/// What a button of `CLIInstalled` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real CLIInstalled content
enum CliInstalledAction {
    /// `onDismissed`
    Dismiss,
}

/// What GitHub Desktop's `CLIInstalled` dialog shows.
#[allow(dead_code)] // filled by the real CLIInstalled content
struct CliInstalledContent {
    title: String,
    /// The footer's buttons: label and what each does.
    buttons: Vec<(String, CliInstalledAction)>,
}

/// Stand-in for GitHub Desktop's `CLIInstalled`
/// (`ui/cli-installed/cli-installed.tsx`) showing `path`. Replace it with
/// the Corvene function once there is one and remove the `#[ignore]`.
fn cli_installed(_path: &Path) -> CliInstalledContent {
    unimplemented!(
        "Corvene has no CLIInstalled content: SimpleDialog draws Popup::CLIInstalled inline (GPUI)"
    )
}

// GHD: unit/ui/small-action-and-dialog-surfaces-test.tsx › small action and dialog surfaces › renders the no-step check-run state and invokes the external-view callback
#[test]
#[ignore = "ghd: missing: no CICheckRunNoStepItem content (ci-check-run-no-steps.tsx); ci_check_popover.rs draws the no-steps pane inline as a GPUI element"]
fn renders_the_no_step_check_run_state_and_invokes_the_external_view_callback() {
    let view = ci_check_run_no_step_item();

    // `getByRole('link', { name: 'View check details' })`, clicked once
    assert_eq!(
        view.button,
        (
            "View check details".to_string(),
            CheckRunNoStepAction::ViewCheckExternally
        )
    );
    // `getByText(…, { exact: false })`
    assert!(
        view.text
            .contains("There are no steps to display for this check.")
    );
    assert!(view.image_alt.is_some());
    assert_eq!(view.image_alt.as_deref(), Some(""));
}

// GHD: unit/ui/small-action-and-dialog-surfaces-test.tsx › small action and dialog surfaces › renders the cli-installed dialog and dismisses through the default button
#[test]
#[ignore = "ghd: deviation: Window / menus › Install Command Line Tool… links /usr/local/bin/corvene as GHD does with github: cli::install_path() is /usr/local/bin/corvene (Linux ~/.local/bin/corvene)"]
fn renders_the_cli_installed_dialog_and_dismisses_through_the_default_button() {
    // `screen.getByText('/usr/local/bin/github')`: the dialog shows the
    // path `Dispatcher::install_cli` linked
    assert_eq!(install_path(), Path::new("/usr/local/bin/github"));
}

// GHD: unit/ui/small-action-and-dialog-surfaces-test.tsx › small action and dialog surfaces › renders the cli-installed dialog and dismisses through the default button
#[test]
#[ignore = "ghd: missing: no CLIInstalled content (cli-installed.tsx); SimpleDialog draws Popup::CLIInstalled's title, text and Ok button inline as GPUI elements"]
fn renders_the_cli_installed_dialog_and_dismisses_through_the_default_button_content() {
    let dialog = cli_installed(&install_path());

    assert_eq!(
        dialog.title,
        if cfg!(target_os = "macos") {
            "Command Line Tool Installed"
        } else {
            "Command line tool installed"
        }
    );
    // `getByRole('button', { name: 'Ok' })`, clicked once
    let ok = dialog
        .buttons
        .iter()
        .find(|(label, _)| label == "Ok")
        .map(|(_, action)| *action);
    assert_eq!(ok, Some(CliInstalledAction::Dismiss));
}
