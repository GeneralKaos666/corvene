# Releasing Corvane

How a tagged commit becomes the artefacts the self-updater and the Homebrew
cask consume (Linux: see [Linux](#linux)). Nothing here needs an Apple Developer ID: the
bundle is signed with a self-signed certificate, and the self-updater checks
a download against the sha256 GitHub lists for the asset.

## Code-signing certificate (one-time, maintainer's machine)

macOS lets an app read its Keychain items (the account tokens,
`corvane_platform::keychain`) without asking only while its designated
requirement matches the one that created them. An ad-hoc signature's
requirement is the binary's hash, so every build and every update would ask
for the login password again. Corvane is therefore signed with a
self-signed certificate: its requirement, `identifier
"com.wasimaster.corvane" and certificate leaf = H"…"`, stays the same across
builds. Gatekeeper treats it like an ad-hoc signature (hence the cask's
quarantine-clearing `postflight_steps`).

```bash
packaging/signing-cert.sh create   # → login keychain, ~/.corvane-signing/corvane-signing.{p12,password}
```

- Keep the `.p12` and its password in the password manager and add them as
  the GitHub Actions secrets `MACOS_SIGNING_P12` (base64 of the file) and
  `MACOS_SIGNING_P12_PASSWORD`. On another machine:
  `packaging/signing-cert.sh import corvane-signing.p12`.
- `packaging/bundle.sh` signs with it whenever the keychain has it (the
  first run asks to use the key: Always Allow), else ad-hoc;
  `CORVANE_SIGN_IDENTITY=-` forces ad-hoc. `packaging/release.sh` refuses a
  bundle that is not signed with it (`ALLOW_ADHOC=1` for tests).
- Losing it is not fatal: a new certificate means
  one more password prompt per Keychain item on every install. The first
  self-signed release does the same for installs of ad-hoc builds.
- `codesign -d -r- /Applications/Corvane.app` shows the requirement.

## Release checklist

1. Bump `version` in `Cargo.toml` (`[workspace.package]`), run
   `packaging/acknowledgements.sh` if dependencies changed, commit,
   `git tag v<version>`. Pushing the tag runs steps 2, 3 and 5 in CI
   ([Releasing from CI](#releasing-from-ci)); the steps below are the local
   path.
2. `packaging/release.sh` (see `packaging/release.sh --help`): release build,
   `packaging/bundle.sh release`, the universal binary when both target
   directories exist, `Corvane-<version>-macos-universal.zip` (+ `.dmg`), the
   `Corvane-Full-…` variant when `FULL=1`, and the cask's sha256. Output
   lands in `target/release-assets/`.
3. Create the GitHub release for the tag and upload the `.zip` and `.dmg`
   files in `target/release-assets/`. The self-updater reads
   `GET /repos/wasi-master/corvane/releases/latest`, picks the `.zip` whose
   name contains `universal` (else the machine's architecture, else `macos`)
   and checks the download against the asset's `digest` (the sha256 GitHub
   computes at upload and shows next to the asset). The release body is
   Markdown; list items tagged `[New]` / `[Improved]` / `[Fixed]` /
   `[Added]` / `[Removed]` become the Release Notes dialog's entries
   (`corvane_core::release_notes`).
4. Update `packaging/homebrew/Casks/corvane.rb` with the version, the
   sha256 `release.sh` printed and the two AppImages' sha256
   (`packaging/homebrew/stamp.py`), and push it to the `homebrew-corvane`
   tap (`packaging/homebrew/README.md`).
5. When a pack changed: [Packs](#packs).

## Packs

The on-demand packs (`crates/corvane-packs`: the tree-sitter grammars, one
archive per OS and architecture) are not assets of the app's releases. They
live on one rolling release tagged `packs`, which is created with
`--latest=false` so `releases/latest` (the updater's feed, the README's
download link) never points at it, next to `packs-manifest.json`. The app
reads `…/releases/download/packs/packs-manifest.json`; every entry carries
the archive's URL and sha256, and the download is checked against it.

A pack is installed by version, so a published `(pack, version, platform)`
is never built or uploaded again: bump the pack's version
(`corvane_grammars::PACK_VERSION`) to ship new grammars. The manifest keeps
the older entries (with their `min_app`) for the apps that still need them.

By hand: `packaging/packs.sh` on each platform (`PACK_OS=android` for the
Android ones), add the `packs` entries of its `packs-manifest.json` to the
release's, then upload the archives and the merged manifest:

```bash
gh release create packs --latest=false --title Packs --notes "…"   # once
gh release upload packs --clobber target/release-assets/packs/*.zip packs-manifest.json
```

## Releasing from CI

`.github/workflows/release.yml` runs on a `v*` tag push. It runs no fmt,
clippy or tests (`ci.yml` does on every push to `main`): tag a commit whose
CI run is green. Every platform builds at once and hands its assets on as
workflow artifacts:

- `plan`: the version (fails when the tag is not `v<Cargo.toml version>`)
  and which packs the `packs` release lacks.
- `macos-build`: one `macos-26` runner (Xcode 26; its `actool` crashes on a
  macOS 15 host) per architecture and variant, a `--no-default-features`
  release build each (`--features full` for `Corvane-Full`).
- `macos-assets`: `packaging/release.sh` with `SKIP_BUILD=1` on those four
  binaries (lipo, bundle signed with the certificate from
  `packaging/signing-cert.sh ci`, zip, dmg), per variant once for the
  universal bundle and once per architecture (`ARCH=arm64` / `x86_64`:
  `Corvane-<version>-macos-arm64.zip`, a smaller download). The updater and
  the cask take the universal zip.
- `linux` (x86_64 and aarch64, and cross-compiled i686 and armhf) and
  `android`.
- `packs`: only the platforms `plan` listed; `publish-packs` uploads them to
  the `packs` release and merges their entries into its manifest.
- `publish`: once every build is through, a **draft** release
  with the installers (`.zip`, `.dmg`, `.deb`, `.AppImage`, signed `.apk`),
  its notes ending in a Downloads table of them
  (`packaging/release-table.py`: platforms as rows, architectures as
  columns, one short link per asset),
  and the `cask` artifact: `corvane.rb` with the version and every sha256
  filled in (also in the run summary).

Repository secrets: `MACOS_SIGNING_P12`, `MACOS_SIGNING_P12_PASSWORD`, and
optionally `CORVANE_GITHUB_CLIENT_SECRET` (without it the `307-sign-in-flow`
"auto" default uses the device flow) and the Android keystore's.

After the run: write the release notes in the draft above its `## Downloads`
section (the Release Notes dialog skips that section; a later run rewrites
it and keeps the rest), publish it (the
self-updater ignores drafts), then copy the `cask` artifact's `corvane.rb`
to the tap.
"Run workflow" with `linux_release` set to a published release's tag
(`gh workflow run release.yml --ref main -f linux_release=v0.1.0`) skips
macOS and Android, builds the Linux assets from that branch, replaces them
on the release and stamps the whole cask (the macOS hash from the release's
zip). `Cargo.toml` must still carry that version. The tag stays where it is,
so the Linux binaries are newer than the tagged source.

"Run workflow" by hand builds the same assets as workflow artifacts without
a release, and on `main` it is also what saves the dependency caches the tag
runs read (a tag's run can read `main`'s caches but not another tag's) (`full` input: skip the Full variant; `publish_packs`: rebuild
every pack and publish it to the `packs` release anyway).

## How the updater uses the assets

`corvane_platform::updater` (checked at launch with a 15–60 s jitter and
every four hours, release builds only):

1. `GET …/releases/latest` (no token; `CORVANE_UPDATE_FEED` overrides the
   URL for testing). A tag newer than the running version by semver wins;
   drafts are skipped.
2. `<zip>` is downloaded to `~/Library/Caches/Corvane/updates/` and its
   sha256 compared with the asset's `digest` from the feed. A release whose
   asset lists no sha256 digest is refused.
3. The "Corvane N is available" banner appears. "Install and Restart" (banner,
   Release Notes dialog, About) unpacks the zip with `ditto` (keeps the
   signature), renames the running bundle to `Corvane.app.old`, moves
   the new bundle in, schedules `open -n` for after this process exits and
   quits. The next launch deletes `Corvane.app.old`.
4. A bundle under `/opt/homebrew/Caskroom`, `/usr/local/Caskroom`, or in
   `/Applications` while a `corvane` cask is installed, is never swapped: the
   banner says `brew upgrade corvane`.

Files the app downloads itself carry no quarantine attribute, so the self-signed
update launches without Gatekeeper's "Open Anyway" dance.

## Linux

`packaging/linux/package.sh` builds two assets from a release build into
`target/linux/`:

- `corvane_<version>_amd64.deb` / `corvane_<version>_arm64.deb`
- `Corvane-<version>-x86_64.AppImage` / `Corvane-<version>-aarch64.AppImage`

for the architecture of the machine it runs on (release.yml builds both,
the arm64 ones on an `ubuntu-24.04-arm` runner).

`TARGET=i686-unknown-linux-gnu` or `TARGET=armv7-unknown-linux-gnueabihf`
cross-compiles the 32-bit ones (`_i386.deb` / `-i686.AppImage`,
`_armhf.deb` / `-armhf.AppImage`): release.yml does on the runner of the
same family, with the target's gcc and the `:i386` / `:armhf` development
libraries from Debian multiarch (the linkers are in `.cargo/config.toml`).
Those two jobs may fail without holding up a release: nothing upstream is
tested on 32-bit Linux. They have no tree-sitter packs and no Homebrew
cask.

A `v<version>` tag does this in release.yml's `linux` jobs; `publish` adds
both to the draft release. The machine's tree-sitter packs
(`tree-sitter-{all,rest}-<v>-linux-<arch>.zip`, `.so` units; `packs.sh`
builds shared objects on Linux) come from the `packs` jobs
([Packs](#packs)).

Only the AppImage updates itself. The updater picks
`Corvane-*-<arch>.AppImage` (the machine's `x86_64` / `aarch64` / `i686` /
`armhf`) from the
latest release, downloads it to `$XDG_CACHE_HOME/corvane/updates/`
(`~/.cache/corvane/updates/`) and checks its sha256. "Install and Restart"
copies the image next to the running one (`$APPIMAGE`, which must be a file
in a folder the user can write), checks that copy again, makes it executable, fsyncs it, renames it over
`$APPIMAGE` and starts it once the old process has exited.

The AppImage also installs its own desktop entry
(`corvane_platform::desktop_entry`, run from `url_schemes::register` at
every launch with `$APPIMAGE` set): the packaged
`com.wasimaster.corvane.desktop` with `Exec=<$APPIMAGE> %U` and
`TryExec=<$APPIMAGE>` goes to `$XDG_DATA_HOME/applications/`, the two icons
from the mounted image (`$APPDIR/usr/share/icons/hicolor`) to
`$XDG_DATA_HOME/icons/hicolor/{256x256,scalable}/apps/`, and `xdg-mime` then
makes it the `x-corvane` / `x-corvane-auth` handler. The entry carries
`X-Corvane-Installer=appimage`: only an entry with that line is rewritten
(when the image moves or the template changes), a user entry without it and
the `.deb`'s entry under `$XDG_DATA_DIRS` are left alone, and with the
`.deb`'s entry present nothing is written. A launch that is not from an
AppImage removes the entry once its image is gone. `package.sh` must keep
shipping the icons at those paths in the AppDir.

A `.deb` install (`/usr/lib/corvane`) is never updated in place: like a
Homebrew cask on macOS, the banner and About only say that the release is
available and to update with the package manager.

The Homebrew cask installs the same AppImage as
`~/Applications/Corvane.AppImage`. An image in `~/Applications` while a
`corvane` cask exists (`Caskroom/corvane` under `$HOMEBREW_PREFIX`,
`/home/linuxbrew/.linuxbrew` or `~/.linuxbrew`) is the cask's and is never
swapped: the banner says `brew upgrade corvane`. The
`publish` job stamps both AppImages' sha256 into the cask.

## Android

`packaging/android/build.sh release` builds the `foss` and `play` packages
with every ABI (arm64-v8a, armeabi-v7a, x86_64, x86) in each; `PER_ABI=1`
also one per ABI. release.yml uploads all ten as
`Corvane-<version>-android-<foss|play>-<arm64|armv7|x86_64|x86|universal>.apk`; the
`play` bundle (`.aab`) stays a workflow artifact for Google Play.

They are signed with the key from `packaging/android/keystore.sh create`
(`~/.corvane-signing/corvane-android.jks` and `.password`; needs a JDK's
`keytool`, so set `JAVA_HOME`). `keystore.sh secrets` stores it as the
`CORVANE_ANDROID_KEYSTORE_BASE64`, `CORVANE_ANDROID_KEYSTORE_PASSWORD`,
`CORVANE_ANDROID_KEY_ALIAS` and `CORVANE_ANDROID_KEY_PASSWORD` repository
secrets; `eval "$(packaging/android/keystore.sh env)"` signs a local
release build. Without them the packages are unsigned and stay workflow
artifacts. Android updates a package only with one signed by the same key:
keep the keystore and its password in the password manager, a lost key
means every user uninstalls first.

## Windows

`packaging/windows/package.ps1` builds the installer from a release build
into `target\windows\`:

- `Corvane-<version>-<x86_64|aarch64|i686>-setup.exe`
- `Corvane-Full-<version>-<arch>-setup.exe` with `FULL=1` (every tree-sitter
  grammar compiled in)

for the architecture of the machine it runs on, or for `TARGET=<Rust
target>`. It needs Inno Setup 6
(`iscc.exe`; `ISCC` names it when it is not in its default folder). The
installer (`packaging/windows/corvane.iss`) is per user and asks for no
administrator rights: `%LOCALAPPDATA%\Programs\Corvane\corvane.exe`, the
command line tool `bin\corvane.bat` with its folder on the user's `PATH`,
a Start menu shortcut (whose AppUserModelID notifications are shown
under), the `x-corvane` and `x-corvane-auth` URL schemes and an
uninstaller.

A `v<version>` tag does this in release.yml's `windows` jobs, one per
architecture and variant (x86_64 and aarch64 natively, i686 cross-compiled
on the x86_64 runner), and `publish` adds the installers to the draft
release with the other platforms' assets. A failure there does not hold up
the other platforms.

The installers are Authenticode-signed only when there is a certificate:
`SIGN_PFX=<file>` and `SIGN_PFX_PASSWORD` make `package.ps1` sign
`corvane.exe` and the installer with `signtool`; in release.yml they come
from the `WINDOWS_SIGNING_PFX_BASE64` (the `.pfx`, base64) and
`WINDOWS_SIGNING_PFX_PASSWORD` repository secrets. Corvane has no
certificate yet (one is bought from a certificate authority), so SmartScreen
warns about the download and a PC with Smart App Control on refuses it.

The `packs` job builds the Windows tree-sitter packs
(`tree-sitter-{all,rest}-<v>-windows-<x86_64|aarch64>.zip`, DLL units) on
Windows runners: `packaging/packs.sh` in Git Bash, with LLVM's clang. By hand that
needs Python as `python` (or `PYTHON=<exe>`), clang on the `PATH` and the
tree-sitter CLI (`CORVANE_TREE_SITTER`).

An installed Corvane updates itself. The updater picks
`Corvane-*-<arch>-setup.exe` from the latest release, downloads it to
`%LOCALAPPDATA%\Corvane\Cache\updates\` and checks it against the sha256
GitHub lists for the asset. "Install and Restart" copies the installer,
checks the copy, quits,
and a hidden PowerShell runs the copy with `/VERYSILENT` once Corvane has
exited and then starts the new Corvane. A Corvane that was not installed by
the installer (`cargo run`, an unpacked copy: no `unins000.exe` next to it)
only says that the release is available.

`packaging/windows/corvane.ico` is generated from the app icon by
`packaging/windows/make-ico.ps1`; run it again when the icon changes.

## Testing the flow locally

```bash
cargo build -p corvane && packaging/bundle.sh
# a "new" version: copy the bundle, bump CFBundleShortVersionString, re-sign, zip
cp -R target/bundle/Corvane.app /tmp/new/ && /usr/libexec/PlistBuddy -c 'Set :CFBundleShortVersionString 9.9.9' /tmp/new/Corvane.app/Contents/Info.plist
codesign --force --sign "${CORVANE_SIGN_IDENTITY:--}" /tmp/new/Corvane.app
(cd /tmp/new && ditto -c -k --sequesterRsrc --keepParent Corvane.app Corvane-9.9.9-macos-universal.zip)
shasum -a 256 /tmp/new/Corvane-9.9.9-macos-universal.zip
# a fake feed: latest.json with tag_name "v9.9.9" and one asset: its
# browser_download_url at http://127.0.0.1:8765/ and "digest": "sha256:<that hash>"
(cd /tmp/new && python3 -m http.server 8765)
CORVANE_UPDATE_CHECK=1 CORVANE_UPDATE_FEED=http://127.0.0.1:8765/latest.json open -n target/bundle/Corvane.app
```

`CORVANE_POPUP=update-available[:brew]` shows the banner, About and Release
Notes with a sample update without any feed.
