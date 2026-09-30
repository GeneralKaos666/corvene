<p align="center"><img src="assets/icon/Corvane-1024.png" width="128" alt="Corvane app icon"></p>

# Corvane

A native, fast, low-memory GitHub Desktop clone written in Rust.

The UI is a one-to-one recreation of GitHub Desktop 3.6.6: same layout, buttons, menus, dialogs and workflow. The engine is different: GPUI (Zed's GPU-accelerated UI framework) for rendering, gitoxide for in-process git reads, and the git CLI for writes so behaviour matches GitHub Desktop exactly.

Status: pre-release. Feature-complete with GitHub Desktop 3.6.6 on macOS, minus a few gaps. No binary release yet. Windows and Linux are planned.

## Requirements

- macOS 15 or newer (Apple Silicon or Intel)
- `git` 2.40+ on your `PATH` (Xcode Command Line Tools or Homebrew)

## Building

```bash
cargo run -p corvane
```

Development builds compile Metal shaders at runtime so a full Xcode install is not required. Release builds in CI use precompiled shaders (`--no-default-features`).

## Install

Homebrew (recommended; the cask clears the quarantine attribute, so Gatekeeper does not block the unnotarized app):

```bash
brew install --cask wasi-master/corvane/corvane
```

Direct download from GitHub Releases: after the first launch is blocked, open System Settings → Privacy & Security and click "Open Anyway", or run `xattr -d com.apple.quarantine /Applications/Corvane.app`.

## License

MIT. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

## Trademarks

Corvane is an independent project. It is not affiliated with, sponsored by or endorsed by GitHub, Inc. GitHub® and GitHub Desktop are trademarks of GitHub, Inc.; they are used here only to describe what Corvane recreates and works with. Corvane does not ship GitHub's logos (the Invertocat or Octocat marks).
