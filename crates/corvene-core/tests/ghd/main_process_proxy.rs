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
//! Corvene's is `corvene_core::integrations::show_folder_contents(path,
//! dependencies)` with `ShowFolderContentsDependencies` (GitHub Desktop's
//! `IShowFolderContentsDependencies`; `Dispatcher::show_repository` decides
//! with the same logic and asks with a native alert). A rejected promise is
//! `Err`; dependencies that throw return `Err`.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use corvene_core::integrations::{
    FileInformation, ShowFolderContentsDependencies, show_folder_contents,
};

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
fn opens_a_conclusively_safe_directory_directly() {
    let (calls_made, dependencies) = create_dependencies();

    show_folder_contents(Path::new("/safe/repository"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(0, 1, 0));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › does nothing when the user cancels for an application bundle
#[test]
fn does_nothing_when_the_user_cancels_for_an_application_bundle() {
    let (calls_made, mut dependencies) = create_dependencies();
    dependencies.is_application_bundle = Box::new(|_| Ok(true));

    show_folder_contents(Path::new("/Applications/Repository.app"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(1, 0, 0));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › reveals an application bundle after confirmation
#[test]
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
fn warns_when_application_metadata_cannot_be_read() {
    let (calls_made, mut dependencies) = create_dependencies();
    dependencies.is_application_bundle = Box::new(|_| Err("metadata unavailable".to_string()));

    show_folder_contents(Path::new("/unknown/repository"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(1, 0, 0));
}

// GHD: unit/main-process-proxy-test.ts › showFolderContents › warns when file information cannot be read
#[test]
fn warns_when_file_information_cannot_be_read() {
    let (calls_made, mut dependencies) = create_dependencies();
    dependencies.stat = Box::new(|_| Err("file information unavailable".to_string()));

    show_folder_contents(Path::new("/unknown/repository"), &dependencies).unwrap();

    assert_eq!(*calls_made.borrow(), calls(1, 0, 0));
}
