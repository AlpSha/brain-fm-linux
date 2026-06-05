# Brain.fm for Linux (unofficial)

A small [Tauri](https://tauri.app) desktop wrapper around the Brain.fm web player
(`https://my.brain.fm`) for Linux — built because the official download page only
ships Mac and Windows apps.

It is a thin native shell around the same web app the official desktop clients
use, plus Linux desktop integration:

- **MPRIS / media keys** — Play/Pause/Next/Previous from your keyboard media keys
  and the desktop media widget (KDE, GNOME, waybar, etc.). Registers as
  `org.mpris.MediaPlayer2.brainfm`.
- **System tray** — left-click toggles the window; right-click menu has
  Show/Hide, Play/Pause and Quit.
- **Close-to-tray** — the window's close button hides to the tray instead of
  quitting; quit from the tray menu.

> You still need a Brain.fm account/subscription. This wrapper only provides the
> desktop shell — audio streams from Brain.fm's servers and requires login. It
> does **not** bypass the paywall.

## Install (Arch / CachyOS)

From the [AUR](https://aur.archlinux.org/packages/brain-fm):

```sh
paru -S brain-fm     # or: yay -S brain-fm
```

Then launch **Brain.fm** from your app launcher or run `brain-fm`.

### Build from source without an AUR helper

```sh
cd aur
makepkg -si          # downloads the v0.1.0 release tarball, builds, installs
```

### Dependencies

Runtime: `webkit2gtk-4.1`, `gtk3`, `libayatana-appindicator`,
`hicolor-icon-theme`. Build: `cargo`/`rust`, `pkgconf`. (`makepkg` lists these in
`depends`/`makedepends`.)

## Develop

The `tauri` CLI is installed locally as a dev dependency:

```sh
npx tauri dev      # run with hot-attach + devtools
npx tauri build    # produce .deb / .AppImage bundles instead of the AUR pkg
```

A plain `cargo build` inside `src-tauri/` also works — no Node/CLI required for a
release build; `makepkg` uses exactly that path.

## Project layout

| Path | What |
|------|------|
| `src-tauri/src/lib.rs` | App setup: window, tray, close-to-tray, MPRIS bridge (`souvlaki`). |
| `src-tauri/src/control.js` | Injected into the page; drives playback + reports state to MPRIS. |
| `src-tauri/tauri.conf.json` | Tauri config (window built in code, remote URL). |
| `src-tauri/capabilities/default.json` | Allows the `my.brain.fm` origin to call the IPC command. |
| `packaging/brain-fm.desktop` | Desktop launcher entry. |
| `aur/PKGBUILD` + `aur/.SRCINFO` | AUR package; builds from the tagged release tarball. |

## If media keys don't control playback

Brain.fm's logged-in player DOM isn't public, so `control.js` uses a layered
strategy: MediaSession state → `<audio>/<video>` element → clicking a
play/pause button → spacebar. If your media keys register but don't toggle
playback, open the player, inspect the real play/pause button, and add its
selector to `SELECTORS.toggle` in
[`src-tauri/src/control.js`](src-tauri/src/control.js), then rebuild. The
MediaSession metadata path (track title in the widget) works without any tweak
if Brain.fm sets `navigator.mediaSession.metadata` (most web players do).

Tip — see what the widget receives:

```sh
busctl --user get-property org.mpris.MediaPlayer2.brainfm \
  /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player PlaybackStatus
# or, with playerctl installed:
playerctl -p brainfm metadata; playerctl -p brainfm play-pause
```

## Cutting a new release

Releases are automated. To ship a new version:

```sh
scripts/release.sh patch      # or: minor | major | 1.2.3
```

That bumps the version in `tauri.conf.json`, `Cargo.toml`, `Cargo.lock` and
`aur/PKGBUILD`, commits, tags `vX.Y.Z`, and (after a confirmation prompt) pushes.

Pushing the tag triggers [`.github/workflows/release.yml`](.github/workflows/release.yml), which:

1. creates the GitHub release (auto-generated notes),
2. computes the release-tarball `sha256` and finalizes `aur/PKGBUILD`,
3. regenerates `.SRCINFO` and **pushes the package to the AUR**,
4. syncs the finalized `PKGBUILD`/`.SRCINFO` back to `master`.

### One-time CI setup

The workflow needs an SSH key authorized on your AUR account, stored as the
`AUR_SSH_PRIVATE_KEY` repo secret:

```sh
# 1. dedicated CI key (no passphrase)
ssh-keygen -t ed25519 -f ~/.ssh/aur_ci -N "" -C "brain-fm-ci"

# 2. add the PUBLIC key to your AUR account (Account → "SSH Public Key", one per line):
cat ~/.ssh/aur_ci.pub

# 3. store the PRIVATE key as a GitHub Actions secret:
gh secret set AUR_SSH_PRIVATE_KEY -R AlpSha/brain-fm-linux < ~/.ssh/aur_ci
```

You can reuse your existing AUR key instead of a dedicated one, but a separate
CI key is easy to revoke.

## Notes / caveats

- Unofficial and not affiliated with Brain.fm. Wrapping their web app may be
  subject to their Terms of Service — use for personal convenience.
- Social logins (Apple/Google/Facebook) open auth popups; the webview allows
  them via `on_new_window` so the session propagates back to the main window.
- The `WebKit ... preconnectTo` line on startup is a benign WebKitGTK log, not a
  crash. The `Package contains reference to $srcdir` makepkg warning is cosmetic
  (a build-machine-local path baked by tauri-codegen).
