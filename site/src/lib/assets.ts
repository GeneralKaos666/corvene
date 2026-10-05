/**
 * The asset names a release carries (packaging/release.md), for the download
 * links. The Full edition (every tree-sitter grammar built in) is
 * `Corvene-Full-…`, `corvene-full` for the Linux .deb and .rpm. Android has
 * two apps: the Compose one (`Corvene-…`) and the desktop app in a
 * NativeActivity (`Corvene-Legacy-…`, `Corvene-Legacy-Full-…`).
 */

export type Edition = "standard" | "full";
export type AndroidApp = "compose" | "legacy";
export type AndroidAbi = "universal" | "arm64" | "armv7" | "x86_64" | "x86";

export function assetNames(version: string, edition: Edition) {
  const full = edition === "full";
  const app = full ? "Corvene-Full" : "Corvene";
  const pkg = full ? "corvene-full" : "corvene";
  // rpm versions cannot hold "-": a pre-release sorts before its release with "~"
  const rpmVersion = version.replace(/-/g, "~");

  return {
    macos: {
      dmg: (arch: "universal" | "arm64" | "x86_64") => `${app}-${version}-macos-${arch}.dmg`,
      zip: (arch: "universal" | "arm64" | "x86_64") => `${app}-${version}-macos-${arch}.zip`,
    },
    windows: {
      setup: (arch: "x86_64" | "aarch64" | "i686") => `${app}-${version}-${arch}-setup.exe`,
      msi: (arch: "x86_64" | "aarch64" | "i686") => `${app}-${version}-${arch}.msi`,
      portable: (arch: "x86_64" | "aarch64" | "i686") => `${app}-${version}-windows-${arch}-portable.zip`,
    },
    linux: {
      deb: (arch: "amd64" | "arm64" | "i386" | "armhf") => `${pkg}_${version}_${arch}.deb`,
      rpm: (arch: "x86_64" | "aarch64" | "i686" | "armv7hl") => `${pkg}-${rpmVersion}-1.${arch}.rpm`,
      appimage: (arch: "x86_64" | "aarch64" | "i686" | "armhf") => `${app}-${version}-${arch}.AppImage`,
      flatpak: (arch: "x86_64" | "aarch64") => `${app}-${version}-${arch}.flatpak`,
      snap: (arch: "x86_64" | "aarch64") => `${app}-${version}-${arch}.snap`,
      tarball: (arch: "x86_64" | "aarch64" | "i686" | "armhf") => `${app}-${version}-linux-${arch}.tar.gz`,
    },
    android: {
      // the Full edition has no universal APK: it would be several hundred MB
      apk: (abi: AndroidAbi, android: AndroidApp = "compose") =>
        `${android === "legacy" ? (full ? "Corvene-Legacy-Full" : "Corvene-Legacy") : app}-${version}-android-foss-${abi}.apk`,
    },
  };
}
