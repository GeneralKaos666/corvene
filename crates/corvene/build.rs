//! Windows: the executable's icon and version information
//! (`packaging/windows/corvene.rc`). GPUI's own build script embeds the
//! application manifest. macOS: only the frameworks the binary uses.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // macOS: link only the frameworks whose symbols the binary uses. The
    // objc2 crates that gpui-component, accesskit and GPUI pull in with
    // default features name CloudKit, CoreData, CoreLocation, CoreImage
    // and OpenGL, and dyld would load each at every launch (and for every
    // askpass run).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo::rustc-link-arg-bins=-Wl,-dead_strip_dylibs");
    }
    #[cfg(windows)]
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let dir = "../../packaging/windows";
        println!("cargo::rerun-if-changed={dir}/corvene.rc");
        println!("cargo::rerun-if-changed={dir}/corvene.ico");
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        // `1.2.3-beta.1` → `1,2,3,0`
        let mut numbers = version
            .split(['.', '-'])
            .map(|part| part.parse::<u16>().unwrap_or(0));
        let mut next = || numbers.next().unwrap_or(0);
        let commas = format!("{},{},{},0", next(), next(), next());
        embed_resource::compile(
            format!("{dir}/corvene.rc"),
            embed_resource::ParamsMacrosAndIncludeDirs(
                [
                    format!("CORVENE_VERSION=\"{version}\""),
                    format!("CORVENE_VERSION_COMMAS={commas}"),
                ],
                [dir],
            ),
        )
        .manifest_optional()
        .expect("the Windows resources compile");
    }
}
