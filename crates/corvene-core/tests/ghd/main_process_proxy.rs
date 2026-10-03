//! Port of GitHub Desktop's `app/test/unit/main-process-proxy-test.ts`.
//!
//! GitHub Desktop's `showFolderContents(path, dependencies)`
//! (`ui/main-process-proxy.ts`) is Repository › Show in Finder
//! (`app.tsx` `showRepository`): a directory is opened directly
//! (`UNSAFE_openDirectory`, `shell.openPath`) unless, on macOS, it is or
//! may be an application bundle (`isApplicationBundle`), or its file
//! information cannot be read; then a confirmation (`confirmRevealDirectory`)
//! comes first and only a confirmed path is revealed (`showItemInFolder`).
//! Every platform operation is a dependency the cases replace.
//!
//! Corvene's Show in Finder is `Dispatcher::show_in_finder(path, cx)`
//! (`crates/corvene-core/src/integrations.rs`): it reveals every path
//! (`cx.reveal_path`, Linux `corvene_platform::apps::show_item_in_folder`)
//! without opening a directory, checking for an application bundle or
//! asking first, and it takes no dependencies (it needs a GPUI `App`).
//! [`show_folder_contents`] stands in for GitHub Desktop's function; a
//! rejected promise is `Err`. Dependencies that throw return `Err`.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

/// GitHub Desktop's `IFileInformation`.
#[allow(dead_code)] // read by the real `showFolderContents`
struct FileInformation {
    is_directory: bool,
}

/// GitHub Desktop's `IShowFolderContentsDependencies`.
#[allow(dead_code)] // read by the real `showFolderContents`
struct ShowFolderContentsDependencies {
    /// Whether the current platform is macOS.
    is_darwin: bool,
    /// Reads file information for the target path.
    stat: Box<dyn Fn(&Path) -> Result<FileInformation, String>>,
    /// Determines whether a path is a macOS application bundle.
    is_application_bundle: Box<dyn Fn(&Path) -> Result<bool, String>>,
    /// Requests confirmation before revealing a potentially executable path.
    confirm_reveal: Box<dyn Fn() -> Result<bool, String>>,
    /// Opens a directory directly in the platform file manager.
    open_directory: Box<dyn Fn(&Path)>,
    /// Reveals and selects a path in the platform file manager.
    reveal_item: Box<dyn Fn(&Path) -> Result<(), String>>,
}

/// Stand-in for GitHub Desktop's `showFolderContents(path, dependencies)`
/// (`ui/main-process-proxy.ts`).
fn show_folder_contents(
    _path: &Path,
    _dependencies: &ShowFolderContentsDependencies,
) -> Result<(), String> {
    unimplemented!("Corvene has no showFolderContents (ui/main-process-proxy.ts)")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Calls {
    confirmations: u32,
    opens: u32,
    reveals: u32,
}

/// The test's `createDependencies()`: the defaults; a case replaces the
/// fields it overrides.
fn create_dependencies() -> (Rc<RefCell<Calls>>, ShowFolderContentsDependencies) {
    let calls = Rc::new(RefCell::new(Calls::default()));

    let confirmations = calls.clone();
    let opens = calls.clone();
    let reveals = calls.clone();
    let dependencies = ShowFolderContentsDependencies {
        is_darwin: true,
        stat: Box::new(|_| Ok(FileInformation { is_directory: true })),
        is_application_bundle: Box::new(|_| Ok(false)),
        confirm_reveal: Box::new(move || {
            confirmations.borrow_mut().confirmations += 1;
            Ok(false)
        }),
        open_directory: Box::new(move |_| {
            opens.borrow_mut().opens += 1;
        }),
        reveal_item: Box::new(move |_| {
            reveals.borrow_mut().reveals += 1;
            Ok(())
        }),
    };

    (calls, dependencies)
}

fn calls(confirmations: u32, opens: u32, reveals: u32) -> Calls {
    Calls {
        confirmations,
        opens,
        reveals,
    }
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › opens a conclusively safe directory directly
#[test]
#[ignore = "ghd: missing: no showFolderContents (ui/main-process-proxy.ts); Dispatcher::show_in_finder always reveals, never opens a directory or checks for an app bundle"]
fn opens_a_conclusively_safe_directory_directly() {
    let (calls_made, dependencies) = create_dependencies();

    show_folder_contents(Path::new("/safe/repository"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(0, 1, 0));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › does nothing when the user cancels for an application bundle
#[test]
#[ignore = "ghd: missing: no showFolderContents (ui/main-process-proxy.ts); Dispatcher::show_in_finder always reveals, never opens a directory or checks for an app bundle"]
fn does_nothing_when_the_user_cancels_for_an_application_bundle() {
    let (calls_made, mut dependencies) = create_dependencies();
    dependencies.is_application_bundle = Box::new(|_| Ok(true));

    show_folder_contents(Path::new("/Applications/Repository.app"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(1, 0, 0));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › reveals an application bundle after confirmation
#[test]
#[ignore = "ghd: missing: no showFolderContents (ui/main-process-proxy.ts); Dispatcher::show_in_finder always reveals, never opens a directory or checks for an app bundle"]
fn reveals_an_application_bundle_after_confirmation() {
    let (test_calls, mut dependencies) = create_dependencies();
    dependencies.is_application_bundle = Box::new(|_| Ok(true));
    let confirmations = test_calls.clone();
    dependencies.confirm_reveal = Box::new(move || {
        confirmations.borrow_mut().confirmations += 1;
        Ok(true)
    });

    show_folder_contents(Path::new("/Applications/Repository.app"), &dependencies).unwrap();

    assert_eq!(*test_calls.borrow(), calls(1, 0, 1));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › handles a failed reveal after confirmation
#[test]
#[ignore = "ghd: missing: no showFolderContents (ui/main-process-proxy.ts); Dispatcher::show_in_finder always reveals, never opens a directory or checks for an app bundle"]
fn handles_a_failed_reveal_after_confirmation() {
    let (test_calls, mut dependencies) = create_dependencies();
    dependencies.is_application_bundle = Box::new(|_| Ok(true));
    let confirmations = test_calls.clone();
    dependencies.confirm_reveal = Box::new(move || {
        confirmations.borrow_mut().confirmations += 1;
        Ok(true)
    });
    let reveals = test_calls.clone();
    dependencies.reveal_item = Box::new(move |_| {
        reveals.borrow_mut().reveals += 1;
        Err("Finder unavailable".to_string())
    });

    let result = show_folder_contents(Path::new("/Applications/Repository.app"), &dependencies);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(*test_calls.borrow(), calls(1, 0, 1));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › does nothing when confirmation fails
#[test]
#[ignore = "ghd: missing: no showFolderContents (ui/main-process-proxy.ts); Dispatcher::show_in_finder always reveals, never opens a directory or checks for an app bundle"]
fn does_nothing_when_confirmation_fails() {
    let (test_calls, mut dependencies) = create_dependencies();
    dependencies.is_application_bundle = Box::new(|_| Ok(true));
    let confirmations = test_calls.clone();
    dependencies.confirm_reveal = Box::new(move || {
        confirmations.borrow_mut().confirmations += 1;
        Err("Dialog unavailable".to_string())
    });

    let result = show_folder_contents(Path::new("/Applications/Repository.app"), &dependencies);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(*test_calls.borrow(), calls(1, 0, 0));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › warns when application metadata cannot be read
#[test]
#[ignore = "ghd: missing: no showFolderContents (ui/main-process-proxy.ts); Dispatcher::show_in_finder always reveals, never opens a directory or checks for an app bundle"]
fn warns_when_application_metadata_cannot_be_read() {
    let (calls_made, mut dependencies) = create_dependencies();
    dependencies.is_application_bundle = Box::new(|_| Err("metadata unavailable".to_string()));

    show_folder_contents(Path::new("/unknown/repository"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(1, 0, 0));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › warns when file information cannot be read
#[test]
#[ignore = "ghd: missing: no showFolderContents (ui/main-process-proxy.ts); Dispatcher::show_in_finder always reveals, never opens a directory or checks for an app bundle"]
fn warns_when_file_information_cannot_be_read() {
    let (calls_made, mut dependencies) = create_dependencies();
    dependencies.stat = Box::new(|_| Err("file information unavailable".to_string()));

    show_folder_contents(Path::new("/unknown/repository"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(1, 0, 0));
}
