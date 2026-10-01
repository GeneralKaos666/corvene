<p align="center"><img src="assets/icon/Corvane-1024.png" width="128" alt="Corvane app icon"></p>

# Corvane

A native, fast, low-memory [GitHub Desktop](https://github.com/apps/desktop) clone written in [Rust](https://rust-lang.org/).

The UI is a one-to-one recreation of [GitHub Desktop 3.6.6](https://github.com/desktop/desktop/releases/tag/release-3.6.6): same layout, buttons, menus, dialogs and workflow. The engine is different: [GPUI](https://gpui.rs/) ([Zed](https://zed.dev/)'s GPU-accelerated UI framework) for rendering, [gitoxide](https://github.com/gitoxidelabs/gitoxide) for in-process git reads, and the [git](https://git-scm.com/) CLI for writes so behaviour matches GitHub Desktop exactly.

Status: pre-release. Feature-complete with GitHub Desktop 3.6.6 on macOS, minus a few gaps. Linux ([X11](https://en.wikipedia.org/wiki/X_Window_System) and [Wayland](https://wayland.freedesktop.org/), x86_64 and arm64) runs the same app with GitHub Desktop's Linux menus and wording; it is newer and not yet pixel-identical to GitHub Desktop everywhere (expect <5% differences). Binaries for both are on [GitHub Releases](https://github.com/wasi-master/corvane/releases). Windows is planned.

## Requirements

### macOS

- macOS 10.15.7 Catalina or newer on Intel, macOS 11 or newer on [Apple Silicon](https://support.apple.com/en-us/116943)
- `git` 2.40+ on your `PATH` ([Xcode Command Line](https://developer.apple.com/documentation/xcode/installing-the-command-line-tools) Tools or [Homebrew](https://brew.sh/)); the Command Line Tools for macOS 15 and older ship an older git, so use Homebrew's there

### Linux

- A distribution with glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 or later), on x86_64 or arm64
- An X11 or Wayland session, and a [Vulkan](https://www.vulkan.org/) driver (Mesa's drivers work)
- A [Secret Service](https://specifications.freedesktop.org/secret-service-spec/latest/) keyring for signed-in accounts: GNOME Keyring, KWallet or KeePassXC
- `git` 2.40+ on your `PATH`; Ubuntu 22.04 ships an older git, so use the [git-core PPA](https://launchpad.net/~git-core/+archive/ubuntu/ppa) there

## Install

### macOS

[Homebrew](https://brew.sh/) (recommended; the cask clears the quarantine attribute, so [Gatekeeper](https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web) does not block the unnotarized app):

```bash
brew install --cask wasi-master/corvane/corvane
```

Direct download from [GitHub Releases](https://github.com/wasi-master/corvane/releases): after the first launch is blocked, open System Settings → Privacy & Security and click "Open Anyway" (on macOS 14 and older, right-click the app and choose Open), or run `xattr -d com.apple.quarantine /Applications/Corvane.app`.

### Linux

[Homebrew](https://brew.sh/) (x86_64 and arm64; installs the AppImage as `~/Applications/Corvane.AppImage`, `brew upgrade corvane` updates it):

```bash
brew install --cask wasi-master/corvane/corvane
```

Or download the `.deb` or the AppImage for your architecture from [GitHub Releases](https://github.com/wasi-master/corvane/releases).

- Debian and Ubuntu: `sudo apt install ./corvane_<version>_amd64.deb` (`arm64` on ARM). Install a newer `.deb` the same way to update.
- Any distribution: `chmod +x Corvane-<version>-x86_64.AppImage` (`aarch64` on ARM), then run it. The AppImage updates itself in place.

An AppImage (from Homebrew or downloaded) adds itself to the application menu and registers the `x-corvane://` links on first launch: it writes `~/.local/share/applications/com.wasimaster.corvane.desktop` and its icon under `~/.local/share/icons/hicolor`, and updates them when you move the file. It leaves this to [AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher) or appimaged when one of them already added the AppImage. Delete those files to remove the entry (`brew uninstall --zap corvane` does).

File → Install Command Line Tool links `corvane` into `~/.local/bin`, the same command line tool GitHub Desktop has. A Flatpak is not available yet.

## Building

```bash
cargo run -p corvane
```

On Linux, install the build dependencies first (Debian and Ubuntu package names):

```bash
sudo apt install clang mold pkg-config libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libfontconfig-dev libfreetype-dev libvulkan-dev
```

On macOS, development builds compile [Metal](https://developer.apple.com/metal/) shaders at runtime so a full [Xcode](https://developer.apple.com/xcode/) install is not required. Release builds in CI use precompiled shaders (`--no-default-features`).

## License

MIT. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

## Trademarks

Corvane is an independent project. It is not affiliated with, sponsored by or endorsed by GitHub, Inc. GitHub® and GitHub Desktop are trademarks of GitHub, Inc.; they are used here only to describe what Corvane recreates and works with. Corvane does not ship GitHub's logos (the Invertocat or Octocat marks).
