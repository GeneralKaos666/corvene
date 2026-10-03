# Homebrew tap layout

The primary install path on macOS, and one of three on Linux (next to the
`.deb` and the bare AppImage), is a Homebrew cask in a tap
repository named `homebrew-corvene` under the `wasi-master` account. This
folder holds the tap's contents so a release can copy them over:

```
homebrew-corvene/            # github.com/wasi-master/homebrew-corvene
├─ README.md                 # install instructions, kept in the tap itself
└─ Casks/
   └─ corvene.rb             # packaging/homebrew/Casks/corvene.rb
```

Creating the tap repository is a manual step (nothing here creates or pushes
remotes). Once it exists:

```bash
cp packaging/homebrew/Casks/corvene.rb ../homebrew-corvene/Casks/corvene.rb
(cd ../homebrew-corvene && git commit -am "corvene <version>" && git push)
```

## Installing Corvene

```bash
brew install --cask wasi-master/corvene/corvene
```

macOS: Corvene is signed with a self-signed certificate (a hobby project without an
Apple Developer ID), and macOS 15 blocks a quarantined, unnotarized app on
first launch. Homebrew 7 removed `--no-quarantine` and always quarantines cask
downloads, so the cask's `postflight_steps` runs
`xattr -dr com.apple.quarantine /Applications/Corvene.app` itself.

Linux (x86_64 and arm64): the cask's `app_image` stanza moves the release's
AppImage to `~/Applications/Corvene.AppImage` (Homebrew's `appimagedir`) and
makes it executable. Homebrew adds no desktop entry and no `corvene` command.
Start the image from `~/Applications` once: an AppImage without an installed
desktop entry writes its own on every launch
(`~/.local/share/applications/com.wasimaster.corvene.desktop` with `Exec`
naming the image, icons under `~/.local/share/icons/hicolor`;
`corvene_platform::desktop_entry`), which gives the launcher entry and the
`x-corvene://` / `x-corvene-auth://` handlers the browser sign-in needs.
`brew uninstall` leaves those files (an `uninstall` stanza would also remove
them on every `brew upgrade`): the entry's `TryExec` hides the launcher once
the image is gone, `brew uninstall --zap` trashes them, and the next launch
of a `.deb` install removes the entry. With AppImageLauncher or appimaged
integrating the image, their entry is the launcher and Corvene writes none. File → Install Command Line Tool
links `corvene` into `~/.local/bin` (the fixed target name keeps that link
and the desktop entry valid across upgrades). The Vulkan driver, the Secret Service keyring and git
stay the system's (a Homebrew `git` under `/home/linuxbrew/.linuxbrew/bin`
is found too).

Upgrade with `brew upgrade corvene`; the in-app updater recognises a
Homebrew install (macOS: the bundle; Linux: an AppImage in `~/Applications`
while a Caskroom holds `corvene`) and only points at that command.

## Updating the cask for a release

The cask carries four hashes: `arm` and `intel` (the same universal macOS
zip) and `arm64_linux` / `x86_64_linux` (the AppImages).
`packaging/homebrew/stamp.py` rewrites them and `version`:
`packaging/release.sh` prints the zip's sha256 and, with `UPDATE_CASK=1`,
stamps the version and the macOS half; release.yml's `cask` job adds the
AppImage hashes and uploads the finished file as the `cask` artifact. By hand:

```bash
packaging/homebrew/stamp.py --version <version> --macos <sha256> \
  --x86_64-linux <sha256> --arm64-linux <sha256>
```

Then copy the file to the tap and push. Homebrew 7 only loads casks from a
tap, so check it with `brew style Casks/corvene.rb` in the tap checkout, then
`brew audit --cask --strict --online wasi-master/corvene/corvene` once pushed.
(`brew style` rejects the checked-in placeholder hashes for being identical;
a stamped cask passes.) The audit compares the bundle's
`LSMinimumSystemVersion` with the cask: 0.1.0 was built for macOS 15, so its
cask in the tap carries `depends_on macos: :sequoia` inside `on_macos`;
later bundles start at 10.15, below Homebrew's own floor, and need none.

Actions → Homebrew cask → Run workflow (`.github/workflows/cask.yml`)
installs the pushed cask with Homebrew on both Linux architectures, checks
the AppImage it put in `~/Applications` and uninstalls it.
