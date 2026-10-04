//! Port of GitHub Desktop's
//! `app/test/unit/ui/small-action-and-dialog-surfaces-test.tsx`.
//!
//! - `CICheckRunNoStepItem` (`ui/check-runs/ci-check-run-no-steps.tsx`):
//!   "There are no steps to display for this check.", a "View check
//!   details" button calling `onViewCheckExternally` and a decorative
//!   paper-stack image (`alt=""`). Corvene's counterpart is
//!   `corvene_ui::ci_check_popover::ci_check_run_no_step_item()`, the
//!   content the check run list of `crates/corvene-ui/src/ci_check_popover.rs`
//!   draws (the button opens the check's URL with `Dispatcher::open_url`).
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
//!   addition), and the title and button, read from
//!   `corvene_ui::dialogs::cli_installed(path)`, the content `SimpleDialog`
//!   draws.
//!
//! A click on a button is GitHub Desktop's callback prop; here the content
//! names the action the button runs, which is what `fireEvent.click` then
//! checks. Not ported (React DOM / test scaffolding): the `role="link"`
//! and the `img` element as such, and the `fs-admin` / `ipcRenderer.send`
//! mocks `CLIInstalled` needs to load.

use std::path::Path;

use corvene_platform::cli::install_path;
use corvene_ui::ci_check_popover::{CheckRunNoStepAction, ci_check_run_no_step_item};
use corvene_ui::dialogs::{CliInstalledAction, cli_installed};

// GHD: unit/ui/small-action-and-dialog-surfaces-test.tsx › small action and dialog surfaces › renders the no-step check-run state and invokes the external-view callback
#[test]
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
