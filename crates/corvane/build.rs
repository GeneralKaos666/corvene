//! Windows: the executable's icon and version information
//! (`packaging/windows/corvane.rc`). GPUI's own build script embeds the
//! application manifest.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    #[cfg(windows)]
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let dir = "../../packaging/windows";
        println!("cargo::rerun-if-changed={dir}/corvane.rc");
        println!("cargo::rerun-if-changed={dir}/corvane.ico");
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        // `1.2.3-beta.1` → `1,2,3,0`
        let mut numbers = version
            .split(['.', '-'])
            .map(|part| part.parse::<u16>().unwrap_or(0));
        let mut next = || numbers.next().unwrap_or(0);
        let commas = format!("{},{},{},0", next(), next(), next());
        embed_resource::compile(
            format!("{dir}/corvane.rc"),
            embed_resource::ParamsMacrosAndIncludeDirs(
                [
                    format!("CORVANE_VERSION=\"{version}\""),
                    format!("CORVANE_VERSION_COMMAS={commas}"),
                ],
                [dir],
            ),
        )
        .manifest_optional()
        .expect("the Windows resources compile");
    }
}
