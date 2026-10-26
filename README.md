# OnionHoster

Host local websites and TCP services on Tor from a desktop application.

Status: 1.0.0 for Linux x86_64.

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
- AppImage in-place updates with signature verification, plus manual `.deb` and
  portable tarball upgrades.

## Install

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
sudo apt install ./OnionHoster_1.0.0_amd64.deb
```

AppImage builds are portable: make the file executable and run it. If FUSE 2 is
unavailable, launch with `APPIMAGE_EXTRACT_AND_RUN=1`. The portable tarball is a
Linux x86_64 build for distributions that are not covered by the `.deb` or
AppImage. It is not statically linked and uses the same system WebKitGTK/GTK
dependencies listed above.

```bash
tar -xzf OnionHoster_1.0.0_linux_x86_64.tar.gz
cd OnionHoster_1.0.0_linux_x86_64
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

Release assets include `SHA256SUMS.txt` for verifying downloads:

```bash
sha256sum -c SHA256SUMS.txt
```

## Development

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
