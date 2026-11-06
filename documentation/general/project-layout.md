# Project layout

The repository is a Cargo workspace:

- `porthmos-vfs` — the virtual filesystem contract every protocol
  implements (`FileSystem`, `Protocol`, `Prompter`) and the shared file types.
- `porthmos-lfs` — the local file system as a `FileSystem`.
- `porthmos-ssh` — SSH connections, host keys, `~/.ssh/config` discovery and
  the `ssh` terminal hand-off, shared by the SFTP and SCP protocols.
- `porthmos-tls` — certificate trust shared by the FTPS and WebDAV protocols.
- `protocols/` — one crate per remote protocol:
  - `protocols/sftp` (`porthmos-sftp`) — SFTP.
  - `protocols/scp` (`porthmos-scp`) — SCP, for SSH servers without SFTP.
  - `protocols/ftp` (`porthmos-ftp`) — FTP and FTPS.
  - `protocols/webdav` (`porthmos-webdav`) — WebDAV.
  - `protocols/s3` (`porthmos-s3`) — S3-compatible object storage.
- `porthmos-core` — the UI-free engine: connections, listings, transfers,
  sync, editing, search, history, profiles, bookmarks and settings. A
  frontend drives it by sending commands and receiving events. Each protocol
  is a cargo feature, all on by default, as is `keyring`.
- `porthmos-tui` — the terminal interface, built as the `porthmos`
  binary.
- `porthmos-gui` — the window interface, built as the `porthmos-gui`
  binary. It drives the same `porthmos-core` engine as the terminal app.
- `porthmos-update` — the release manifest, signatures and digests shared by
  the release tool and the updater.
- `porthmos-release` — the tool that packages, signs and verifies release
  assets.
