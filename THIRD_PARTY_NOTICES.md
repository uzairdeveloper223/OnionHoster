# Third-party notices

OnionHoster is licensed under GPL-3.0-or-later. This file records the third-party
components that are bundled with, or required by, OnionHoster. It is not a substitute
for the full license texts shipped by each project.

## Bundled Tor runtime

The downloaded Tor Expert Bundle is installed under `src-tauri/resources/tor/` and is
not stored in Git.

| Component | Version | License | Upstream |
| --- | --- | --- | --- |
| Tor | 0.4.9.14 (Tor Browser 15.0.26 Expert Bundle) | BSD-3-Clause | https://www.torproject.org/ |
| lyrebird (obfs4, snowflake, webtunnel, meek_lite) | 0.8.1 | BSD-3-Clause | https://gitlab.torproject.org/tpo/anti-censorship/pluggable-transports/lyrebird |
| OpenSSL | bundled with Tor | Apache-2.0 | https://www.openssl.org/ |
| libevent | bundled with Tor | BSD-3-Clause | https://libevent.org/ |
| GeoIP database | bundled with Tor | See Tor Project and MaxMind terms | https://www.torproject.org/ |

The exact bundle URL and SHA-256 digest are pinned in `scripts/fetch-tor.sh`. The
download is verified against the Tor Browser signing key with fingerprint
`EF6E286DDA85EA2A4BA7DE684E2C6E8793298290`.

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

## Distribution notes

When distributing OnionHoster binaries, ship this file, the GPL-3.0-or-later license,
and the corresponding source code. The Tor Expert Bundle's own license files remain in
the downloaded bundle. No Tor Project, ngrok, or Cloudflare trademark is claimed.
