# Configuration

Settings, saved connections, and bookmarks live under
`$XDG_CONFIG_HOME/porthmos` (or `~/.config/porthmos` if unset) on Linux and
macOS, and under `%APPDATA%\porthmos` on Windows:

- `config.toml` — panel, transfer, edit, key-binding and theme settings. See
  [`config/config.example.toml`](../../config/config.example.toml) for every
  option with its default.
- `connections.toml` — saved connection profiles.
- `bookmarks.toml` — saved bookmarks.
- `known_certificates.toml` — TLS certificates you chose to trust.

Transfer history and other state live under `$XDG_STATE_HOME/porthmos`
(or `~/.local/state/porthmos`) on Linux and macOS, and under
`%LOCALAPPDATA%\porthmos` on Windows.

If you used this app under its old name, TermConnect, your old
`termconnect` config folder is moved to the new location on first start,
unless a `porthmos` folder already exists (Linux and macOS only).

Logs go to `porthmos.log` in that state folder; set `RUST_LOG` to control
verbosity.
