# OnionHoster

Host local websites and TCP services on Tor from Linux or Android.

Status: 1.0.2. Linux x86_64 and Android arm64 builds.

## Features

- Publish static folders or HTML files, local HTTP/HTTPS servers, and raw TCP
  services as persistent onion services.
- Tor bootstrap progress, logs, persistent bridges, private onions with v3
  client keys, PoW defenses, and max stream limits.
- Optional Cloudflare Quick, Cloudflare Named, and custom tunnels.
- Public chat with live messages, media previews, and persistent local storage.
- Public file sharing with uploads, downloads, video playback, and quotas.
- Encrypted `.onionhoster` backup and restore for onion addresses and service
  settings only.
- Smart update system with in-place signed AppImage updates, plus one-click copyable
  terminal update commands for `.deb` and portable tarball installations, and APK
  downloads on Android.
- Android background hosting with an ongoing notification and a Stop hosting action.

## Install

### Linux

Download the `.deb`, AppImage, or portable tarball from the project releases.
The `.deb` bundles Tor and cloudflared under `/usr/lib/OnionHoster/resources`.
AppImage builds copy their runtime out of the temporary mount into
`~/.local/share/dev.uzair.onionhoster/runtime` on first launch. The portable
tarball keeps the binary and runtime together under `bin/` and `lib/`.

Runtime dependencies are provided by the distribution: WebKitGTK 4.1, GTK 3,
librsvg, ca-certificates, and xdg-utils. Release binaries target glibc 2.35, so
the baseline is Ubuntu 22.04+, Debian 12+, or an equivalent distribution. On
Ubuntu 22.04+:

```bash
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 librsvg2-2 ca-certificates xdg-utils
```

On Fedora:

```bash
sudo dnf install webkit2gtk4.1 gtk3 librsvg2 ca-certificates xdg-utils
```

On Arch:

```bash
sudo pacman -S webkit2gtk-4.1 gtk3 librsvg ca-certificates xdg-utils
```

Install the `.deb` with your package manager so dependencies are resolved:

```bash
sudo apt install ./onionhoster_1.0.2_amd64.deb
```

AppImage builds are portable: make the file executable and run it. If FUSE 2 is
unavailable, launch with `APPIMAGE_EXTRACT_AND_RUN=1`. The portable tarball is a
Linux x86_64 build for distributions that are not covered by the `.deb` or
AppImage. It is not statically linked and uses the same system WebKitGTK/GTK
dependencies listed above.

```bash
tar -xzf OnionHoster_1.0.2_linux_x86_64.tar.gz
cd OnionHoster_1.0.2_linux_x86_64
./bin/onionhoster
```

To install the tarball for the current user instead:

```bash
./install.sh
```

`install.sh` copies the binary to `~/.local/bin/onionhoster`, the runtime to
`~/.local/lib/OnionHoster`, and adds a desktop entry. Use `./uninstall.sh` to
remove those files. Application data remains in
`~/.local/share/dev.uzair.onionhoster` until you delete it.

### Android

Download `OnionHoster_<version>_android_arm64.apk` from the project releases and
open it to install. Requires Android 7.0+ and an arm64 device with an up-to-date
Android System WebView. Allow installation from your browser or file manager
when Android asks. Tor, bridge transports, and cloudflared are bundled in the APK.

Start Tor from the top bar or Settings. Hosting runs as a foreground service
with an ongoing notification and keeps the CPU awake while Tor is active. Allow
notifications to use the Stop hosting action. Battery restrictions vary by
device; allow unrestricted battery use if hosting stops with the screen off.
Force-stopping the app or restarting the device stops hosting. Open the app and
start Tor again to resume autostart services.

Static folders and HTML files are copied into private app storage using the
Android file picker. These are snapshots: later changes to the original files
are not reflected automatically. Stop the service and use Replace site to import
an updated folder or HTML file without changing its onion address. Imports are
limited to 512 MB, 10,000 entries,
and 32 nested folders. Deleting a service removes its unused imported copy.
Local HTTP/HTTPS and TCP targets must be reachable from the Android device;
`127.0.0.1` refers to the phone itself.

Encrypted backups and client keys use the Android document picker. Address
backups do not include imported sites, chat messages, or shared files. Static
paths from another device need a fresh import through Replace site before they
can be hosted.

Settings checks for new APKs and opens the download in your browser. Install the
new APK over the existing app to keep its data. Android requires the same signing
key for every update.

### Checksums

Release assets include `SHA256SUMS.txt` for verifying downloads:

```bash
sha256sum -c SHA256SUMS.txt
```

## Development

The React/TypeScript interface and Rust hosting backend are shared between both
platforms. Android adds a small Kotlin bridge for background hosting and document
access.

### Linux

```bash
pnpm install
pnpm fetch:runtime
pnpm tauri dev
```

`pnpm fetch:runtime` downloads the pinned Tor Expert Bundle and cloudflared
release, verifies their SHA-256 digests (and the Tor Browser signature), and
installs them into the ignored `src-tauri/resources/tor/` and
`src-tauri/resources/cloudflared/` directories. It requires `curl`, `gpg`, `tar`,
and `sha256sum`. The runtime binaries are not committed to Git.

Prerequisites: Node 20+, pnpm 10+, Rust 1.90+, and the Tauri Linux system dependencies. On Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

To associate the development window with the app icon in the GNOME dock, optionally run:

```bash
./scripts/install-dev-desktop.sh
```

This creates a hidden development `.desktop` entry in your user data directory.
Packaged `.deb` and AppImage builds include their own desktop entry and icons.
The portable tarball's `install.sh` creates the same per-user integration.

To create the portable tarball after a release build:

```bash
./scripts/package-tarball.sh
```

The result is written to
`src-tauri/target/release/bundle/tarball/OnionHoster_<version>_linux_x86_64.tar.gz`.

### Android

Prerequisites: Node 20.19+, pnpm 10+, Rust 1.90+, Java 21, Go 1.27.3+, Android SDK
36, build-tools 36.0.0, and NDK 28.2.13676358. The runtime-fetch script currently
uses the Linux NDK toolchain.

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
sdkmanager "platforms;android-36" "build-tools;36.0.0" "ndk;28.2.13676358"
rustup target add aarch64-linux-android
pnpm install
pnpm android:dev
```

Connect an arm64 device with USB debugging enabled. The Android Gradle project is
tracked under `src-tauri/gen/android/`; it does not need to be initialized again.
`pnpm fetch:android` verifies the pinned Android Tor Expert Bundle and builds
cloudflared with Android DNS support. Downloads, native libraries, and build
outputs are ignored by Git. It also requires `curl`, `gpg`, `tar`, and `sha256sum`.

For an x86_64 emulator:

```bash
rustup target add x86_64-linux-android
pnpm fetch:android x86_64
pnpm tauri android dev --target x86_64
```

Build an unsigned arm64 release APK:

```bash
pnpm android:build
```

The APK is written under `src-tauri/gen/android/app/build/outputs/apk/`. To sign
and verify it, set `ANDROID_KEYSTORE_PATH`, `ANDROID_KEY_ALIAS`,
`ANDROID_KEYSTORE_PASSWORD`, and `ANDROID_KEY_PASSWORD`, then run:

```bash
pnpm android:sign
```

The signed result is written to
`src-tauri/target/release/bundle/android/OnionHoster_<version>_android_arm64.apk`.
Keep the release keystore backed up and reuse it for later versions.

## Releases

`.github/workflows/ci.yml` checks the frontend, Rust tests and clippy, shell
syntax, version consistency, and an Android debug APK build.

`.github/workflows/release.yml` builds Linux packages and a signed Android APK in
parallel, then creates one draft release with both platforms, update manifests,
and `SHA256SUMS.txt`. A manual run uploads the combined artifacts without creating
a GitHub release.

Set these repository secrets before releasing:

- `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` for
  AppImage updates. The public key is configured in `src-tauri/tauri.conf.json`.
- `ANDROID_KEYSTORE_BASE64`: the base64-encoded Android release keystore.
- `ANDROID_KEY_ALIAS`, `ANDROID_KEYSTORE_PASSWORD`, and `ANDROID_KEY_PASSWORD`:
  the keystore alias and passwords.

Update the version in `package.json`, `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json`, and `src/lib/app.ts`. Run
`bash scripts/verify-version.sh`, then push the matching `v<version>` tag. The
workflow checks that the tag matches, removes the temporary keystore after
signing, and generates `latest.json` and `android-latest.json`. Review and publish
the draft release to make it available to update checks.

## Third-party notices

Tor and the other bundled components are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

## Author

- GitHub: [uzairdeveloper223](https://github.com/uzairdeveloper223)
- LinkedIn: [uzairmughal001](https://www.linkedin.com/in/uzairmughal001/)
- Website: [uzair.is-a.dev](https://uzair.is-a.dev)

OnionHoster is an independent project. It is not affiliated with, endorsed by, or sponsored by the Tor Project, Cloudflare, or any content hosted by its users.
