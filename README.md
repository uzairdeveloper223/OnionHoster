# OnionHoster

Host local websites and TCP services on Tor from a desktop application.

Status: under active development. Linux preview. No public release yet.

## Development

```bash
pnpm install
pnpm fetch:tor
pnpm tauri dev
```

`pnpm fetch:tor` downloads the pinned Tor Expert Bundle, verifies its SHA-256 digest
and Tor Browser signature, and installs it into the ignored
`src-tauri/resources/tor/` directory. It requires `curl`, `gpg`, `tar`, and
`sha256sum`. The Tor runtime is not committed to Git.

Prerequisites: Node 20+, pnpm 10+, Rust 1.90+, and the Tauri Linux system dependencies. On Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

To associate the development window with the app icon in the GNOME dock, optionally run:

```bash
./scripts/install-dev-desktop.sh
```

This creates a hidden development `.desktop` entry in your user data directory. Packaged
`.deb` and AppImage builds include their own desktop entry and icons.

## Third-party notices

Tor and the other bundled components are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

## Author

- GitHub: [uzairdeveloper223](https://github.com/uzairdeveloper223)
- LinkedIn: [uzairmughal001](https://www.linkedin.com/in/uzairmughal001/)
- Website: [uzair.is-a.dev](https://uzair.is-a.dev)

OnionHoster is an independent project. It is not affiliated with, endorsed by, or sponsored by the Tor Project, ngrok, Cloudflare, or any content hosted by its users.
