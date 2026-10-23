<p align="center">
  <img src="assets/icon/porthmos.svg" alt="Porthmos icon" width="128" height="128">
</p>

# Porthmos

A dual-panel file manager for the terminal: local files in one panel, a
remote connection in the other, with saved connection profiles, background
transfers, search, folder sync and in-place editing. It talks SFTP, SCP, FTP
and FTPS, WebDAV and S3-compatible storage.

## Features

- **Dual-panel browser** — the local filesystem on one side, the active
  connection on the other. Several connections can stay open at once and you
  switch between them with `Ctrl+N` or the number keys. Entries show size,
  modification time, owner and link targets, and the remote panel shows free
  space.
- **Protocols** — SFTP and SCP (over SSH), FTP and FTPS, WebDAV, and S3 and
  S3-compatible stores (AWS, R2, MinIO, B2). SCP is for SSH servers without
  SFTP. SSH host keys go to `~/.ssh/known_hosts`. A new TLS certificate is
  shown to you and remembered in `known_certificates.toml` once you accept
  it; a certificate that has changed since is refused.
- **Saved connections** — host, port, username, identity file, start folder
  and optional password, alongside the hosts read from `~/.ssh/config`.
  Connections can be grouped and tagged, and the list can be filtered. Saved
  passwords and secrets go to the system keyring when there is one; without a
  keyring they are kept only until Porthmos quits.
- **Background transfers** — copy and move, files or whole folders, queued
  and running in parallel. Transfers can be paused, reordered, cancelled and
  speed-limited. They are written to a `.part` file first and resume after a
  cancel, a dropped connection or a restart. A conflict dialog offers
  overwrite, skip, rename or cancel, for one file or for all of them.
- **Folder sync** — compare a local and a remote folder by time or size, one
  way or both ways, review the differences, and run only the ones you tick.
- **Edit remote files** — open a remote file in `$VISUAL`, `$EDITOR` or the
  editor set in `config.toml`, and upload it again when you save. A file that
  changed on the server in the meantime is never overwritten silently.
- **Preview** — see the start of a file without downloading it.
- **Search** — recursive filename search on either side, and a live filter
  (`/`) for the folder you are in.
- **History** — a log of past transfers.
- **Bookmarks** — jump back to a local or remote folder on a given host.
- **Command palette** — `Ctrl+P` finds actions, hosts to connect or
  disconnect, and files in the open folders, by name.
- **Terminal hand-off** — on SFTP and SCP connections, `F4` drops to a full
  `ssh` session in the folder you are looking at, and returns to the browser
  when you exit it.

## Installing

Requires a Rust toolchain supporting the 2024 edition (stable is fine).

```sh
cargo build --release
```

The binary is written to `target/release/porthmos`.

Optional: install [`sshpass`](https://linux.die.net/man/1/sshpass) if you
want the F4 terminal handoff to use a saved profile password automatically,
instead of prompting for it again.

**Platform:** Linux and macOS only — the app relies on Unix file
permissions and process APIs throughout.

## Project layout

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

## Keybindings

Keys belong to a screen. A key is looked up on the current screen first and
then in the global table, so the same key can mean different things on
different screens. The tables below are the defaults.

**Everywhere**

| Key | Action |
| --- | --- |
| `?` / `F1` | Help |
| `q` / `F10` | Quit (asks first if a transfer or an edit is still running) |
| `Esc` | Back / dismiss |
| `↑` `↓` / `k` `j` | Move |
| `Enter` | Open |
| `Space` | Toggle selection |
| `/` | Filter the list |
| `F9` | Connections screen |
| `F4` | Terminal on the active connection |
| `Ctrl+R` | Refresh |
| `Ctrl+C` | Cancel the active transfer |
| `Ctrl+H` | Toggle hidden files |
| `Ctrl+S` | Cycle sort order |
| `Ctrl+D` / `Ctrl+B` | Bookmark here / open bookmarks |
| `Ctrl+F` | Search |
| `Ctrl+T` / `Ctrl+Y` | Transfers / history |
| `Ctrl+U` | Sync |
| `Ctrl+P` | Command palette |
| `Ctrl+N` | Next connection |

**Files**

| Key | Action |
| --- | --- |
| `Tab` | Switch panel |
| `c` / `F5` | Copy |
| `m` / `F6` | Move |
| `n` / `F7` | New folder |
| `d` / `F8` | Delete |
| `r` / `F2` | Rename |
| `e` / `F3` | Edit |
| `p` | Preview |
| `t` | Transfers |
| `1`–`9` | Switch to connection 1–9 |

**Connections:** `n` / `F6` add, `e` / `F2` edit, `x` / `F8` disconnect,
`Delete` delete the saved connection.

**Transfers:** `p` pause, `r` retry, `x` / `F8` remove, `c` clear finished,
`K` / `J` move a row up or down, `Ctrl+L` speed limit.

**History:** `c` / `F8` clear.

Every binding can be changed in `config.toml`. Press `F1` in the app for the
live list — it reflects your configured bindings rather than this table.

## Configuration

Settings, saved connections, and bookmarks live under
`$XDG_CONFIG_HOME/porthmos` (or `~/.config/porthmos` if unset):

- `config.toml` — panel, transfer, edit, key-binding and theme settings. See
  [`config/config.example.toml`](config/config.example.toml) for every
  option with its default.
- `connections.toml` — saved connection profiles.
- `bookmarks.toml` — saved bookmarks.
- `known_certificates.toml` — TLS certificates you chose to trust.

Transfer history and other state live under `$XDG_STATE_HOME/porthmos`
(or `~/.local/state/porthmos`).

If you used this app under its old name, TermConnect, your old
`termconnect` config folder is moved to the new location on first start,
unless a `porthmos` folder already exists.

Logs go to `porthmos.log` in that state folder; set `RUST_LOG` to control
verbosity.

## Icon and brand colors

The icon, `assets/icon/porthmos.svg`, shows a glowing blue strait flowing
between dark, layered land. *Porthmos* (πορθμός) is Greek for "strait".

Run `scripts/icons/generateIcons.sh` to export it as PNGs (16–1024 px) into
`assets/icon/png/`; the PNGs are not committed. The script needs `rsvg-convert` or `resvg`, because the
glow uses a blur filter.

| Color | Hex |
| --- | --- |
| Land (dark charcoal) | `#101012` → `#2b2a30` |
| Water (cyan to blue) | `#4fe3ff` → `#2fd8f8` → `#1aa6f2` → `#1668e6` |

## License

MIT — see [LICENSE](LICENSE).
