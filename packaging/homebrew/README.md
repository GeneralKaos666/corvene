# Homebrew tap layout

The primary install path is a Homebrew cask in a tap
repository named `homebrew-corvane` under the `wasi-master` account. This
folder holds the tap's contents so a release can copy them over:

```
homebrew-corvane/            # github.com/wasi-master/homebrew-corvane
├─ README.md                 # the install instructions below
└─ Casks/
   └─ corvane.rb             # packaging/homebrew/Casks/corvane.rb
```

Creating the tap repository is a manual step (nothing here creates or pushes
remotes). Once it exists:

```bash
cp packaging/homebrew/Casks/corvane.rb ../homebrew-corvane/Casks/corvane.rb
cp packaging/homebrew/README.md ../homebrew-corvane/README.md
(cd ../homebrew-corvane && git commit -am "corvane <version>" && git push)
```

## Installing Corvane

```bash
brew install --cask wasi-master/corvane/corvane
```

Corvane is signed with a self-signed certificate (a hobby project without an
Apple Developer ID), and macOS 15 blocks a quarantined, unnotarized app on
first launch. Homebrew 7 removed `--no-quarantine` and always quarantines cask
downloads, so the cask's `postflight_steps` runs
`xattr -dr com.apple.quarantine /Applications/Corvane.app` itself.

Upgrade with `brew upgrade corvane`; the in-app updater recognises a
Homebrew install and only points at that command.

## Updating the cask for a release

`packaging/release.sh` prints the zip's sha256 and, with `UPDATE_CASK=1`,
rewrites `version` and `sha256` in `packaging/homebrew/Casks/corvane.rb`.
Then copy the file to the tap and push. Homebrew 7 only loads casks from a
tap, so check it with `brew style Casks/corvane.rb` in the tap checkout, then
`brew audit --cask --strict --online wasi-master/corvane/corvane` once pushed.
