//! Port of GitHub Desktop's `app/test/unit/is-application-bundle-test.ts`
//! (`lib/is-application-bundle.ts`).
//!
//! GitHub Desktop reads `mdls -name kMDItemContentType -name
//! kMDItemContentTypeTree <path>` before Show in Finder opens a directory
//! (`ui/main-process-proxy.ts` `showFolderContents`), so that it never
//! launches an application bundle; `isApplicationBundleFromMetadata`
//! interprets that output and throws when it is inconclusive. Corvene's Show
//! in Finder (`Dispatcher::show_in_finder`, `corvene_platform::apps`) reveals
//! every path without looking at its metadata, so there is no equivalent;
//! [`is_application_bundle_from_metadata`] is a stand-in where a thrown
//! error is `Err(message)`.

/// Stand-in for GitHub Desktop's `isApplicationBundleFromMetadata(metadata)`
/// (`lib/is-application-bundle.ts`). Replace it with the Corvene function
/// once there is one and remove the `#[ignore]`s.
fn is_application_bundle_from_metadata(_metadata: &str) -> Result<bool, String> {
    unimplemented!("Corvene has no isApplicationBundleFromMetadata (lib/is-application-bundle.ts)")
}

// GHD: unit/is-application-bundle-test.ts › isApplicationBundleFromMetadata › identifies application bundles
#[test]
#[ignore = "ghd: missing: no isApplicationBundleFromMetadata (lib/is-application-bundle.ts); Show in Finder never reads mdls"]
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
#[ignore = "ghd: missing: no isApplicationBundleFromMetadata (lib/is-application-bundle.ts); Show in Finder never reads mdls"]
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
#[ignore = "ghd: missing: no isApplicationBundleFromMetadata (lib/is-application-bundle.ts); Show in Finder never reads mdls"]
fn rejects_inconclusive_metadata() {
    let result = is_application_bundle_from_metadata("kMDItemContentType = (null)");
    assert!(
        matches!(&result, Err(message) if message.contains("did not conclusively identify a directory")),
        "expected an error matching /did not conclusively identify a directory/, got {result:?}"
    );
}

// GHD: unit/is-application-bundle-test.ts › isApplicationBundleFromMetadata › rejects an unknown primary type that inherits from public.directory
#[test]
#[ignore = "ghd: missing: no isApplicationBundleFromMetadata (lib/is-application-bundle.ts); Show in Finder never reads mdls"]
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
