# Third-party notices

OnionHoster is licensed under GPL-3.0-or-later. This file records the third-party
components that are bundled with, or required by, OnionHoster. It is not a substitute
for the full license texts shipped by each project.

## Bundled Tor runtime

The downloaded Tor Expert Bundle is installed under `src-tauri/resources/tor/` and is
not stored in Git. Android uses the Android Tor Expert Bundle 16.0; its executables
are packaged under the APK's native-library directory and its data and notices
under `src-tauri/resources/android/`.

| Component | Version | License | Upstream |
| --- | --- | --- | --- |
| Tor | 0.4.9.14 (Tor Browser 15.0.26 Expert Bundle) | BSD-3-Clause | https://www.torproject.org/ |
| lyrebird (obfs4, snowflake, webtunnel, meek_lite) | 0.8.1 | BSD-3-Clause | https://gitlab.torproject.org/tpo/anti-censorship/pluggable-transports/lyrebird |
| OpenSSL | bundled with Tor | Apache-2.0 | https://www.openssl.org/ |
| libevent | bundled with Tor | BSD-3-Clause | https://libevent.org/ |
| GeoIP database | bundled with Tor | See Tor Project and MaxMind terms | https://www.torproject.org/ |

The exact bundle URL and SHA-256 digest are pinned in `scripts/fetch-tor.sh`. The
Android URLs and per-architecture digests are pinned in
`scripts/fetch-android-runtime.sh`. Both bundles include their upstream license
texts, including those for OpenSSL, libevent, zlib, zstd, and bridge transports.
The download is verified against the Tor Browser signing key with fingerprint
`EF6E286DDA85EA2A4BA7DE684E2C6E8793298290`.

## Bundled cloudflared

The Cloudflare Tunnel client is installed under `src-tauri/resources/cloudflared/`
and is not stored in Git.

| Component | Version | License | Upstream |
| --- | --- | --- | --- |
| cloudflared | 2026.10.3 | Apache-2.0 | https://github.com/cloudflare/cloudflared |

The exact release URL and SHA-256 digest are pinned in
`scripts/fetch-cloudflared.sh`. cloudflared is used only when the user explicitly
starts a tunnel. OnionHoster is not affiliated with Cloudflare.

Android builds compile the same cloudflared version from a checksum-pinned source
archive using Go and the Android NDK. The source URL and digest are recorded in
`scripts/fetch-android-runtime.sh`; the upstream license is included in the APK.

## Application framework

| Component | License |
| --- | --- |
| Tauri | MIT or Apache-2.0 |
| React and React DOM | MIT |
| Vite | MIT |
| TypeScript | Apache-2.0 |
| Tailwind CSS | MIT |
| Lucide | ISC |
| Zustand | MIT |
| clsx | MIT |
| tailwind-merge | MIT |
| Geist and Geist Mono | SIL Open Font License 1.1 |
| AndroidX | Apache-2.0 |
| Material Components for Android | Apache-2.0 |

## Distribution notes

When distributing OnionHoster binaries, ship this file, the GPL-3.0-or-later license,
and the corresponding source code. The Tor Expert Bundle's own license files remain in
the downloaded bundle. No Tor Project or Cloudflare trademark is claimed.
