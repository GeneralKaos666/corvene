//! Port of GitHub Desktop's `app/test/unit/is-application-bundle-test.ts`
//! (`lib/is-application-bundle.ts`).
//!
//! GitHub Desktop reads `mdls -name kMDItemContentType -name
//! kMDItemContentTypeTree <path>` before Show in Finder opens a directory
//! (`ui/main-process-proxy.ts` `showFolderContents`), so that it never
//! launches an application bundle; `isApplicationBundleFromMetadata`
//! interprets that output and throws when it is inconclusive. Corvene's is
//! `corvene_platform::apps::is_application_bundle_from_metadata` (used by
//! `apps::is_application_bundle`, which Repository › Show in Finder,
//! `Dispatcher::show_repository`, asks), where a thrown error is
//! `Err(message)`.

use corvene_platform::apps::is_application_bundle_from_metadata;

// GHD: unit/is-application-bundle-test.ts › isApplicationBundleFromMetadata › identifies application bundles
#[test]
fn identifies_application_bundles() {
    let metadata = r#"
      kMDItemContentType = "com.apple.application-bundle"
      kMDItemContentTypeTree = (
        "com.apple.application-bundle",
        "public.executable",
        "public.directory"
      )
    "#;

    assert_eq!(is_application_bundle_from_metadata(metadata), Ok(true));
}

// GHD: unit/is-application-bundle-test.ts › isApplicationBundleFromMetadata › identifies non-executable directories
#[test]
fn identifies_non_executable_directories() {
    let metadata = r#"
      kMDItemContentType = "public.folder"
      kMDItemContentTypeTree = (
        "public.folder",
        "public.directory",
        "public.item"
      )
    "#;

    assert_eq!(is_application_bundle_from_metadata(metadata), Ok(false));
}

// GHD: unit/is-application-bundle-test.ts › isApplicationBundleFromMetadata › rejects inconclusive metadata
#[test]
fn rejects_inconclusive_metadata() {
    let result = is_application_bundle_from_metadata("kMDItemContentType = (null)");
    assert!(
        matches!(&result, Err(message) if message.contains("did not conclusively identify a directory")),
        "expected an error matching /did not conclusively identify a directory/, got {result:?}"
    );
}

// GHD: unit/is-application-bundle-test.ts › isApplicationBundleFromMetadata › rejects an unknown primary type that inherits from public.directory
#[test]
fn rejects_an_unknown_primary_type_that_inherits_from_public_directory() {
    let metadata = r#"
      kMDItemContentType = "com.example.package"
      kMDItemContentTypeTree = (
        "com.example.package",
        "public.directory",
        "public.item"
      )
    "#;

    let result = is_application_bundle_from_metadata(metadata);
    assert!(
        matches!(&result, Err(message) if message.contains("did not conclusively identify a directory")),
        "expected an error matching /did not conclusively identify a directory/, got {result:?}"
    );
}
