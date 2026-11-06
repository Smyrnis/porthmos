# Configuration

Settings, saved connections, and bookmarks live under
`$XDG_CONFIG_HOME/porthmos` (or `~/.config/porthmos` if unset) on Linux and
macOS, and under `%APPDATA%\porthmos` on Windows:

- `config.toml` — panel, transfer, edit, terminal, key-binding and theme
  settings. See
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

## Terminal for the window app

`[terminal] command` in `config.toml` chooses the terminal program the window
app opens for `F4` and for editors that run in a terminal. Empty means
`$TERMINAL`, then `x-terminal-emulator`, then a list of common terminals. Write
the program with any arguments (`kitty --single-instance`). Put `{command}`
where the command to run belongs when the program needs it somewhere other
than the end (`wt.exe new-tab {command}`). The setting is also used on macOS
and Windows. The Settings screen has a Terminal row that applies at once. The
terminal app runs in the terminal you started it in and ignores the setting.

## What Porthmos remembers

Between runs Porthmos keeps, in the state folder:

- the last folder of each saved connection (`last_used.toml`, next to its
  last-used time). A connection with a start folder set still opens there; the
  remembered folder is used when none is set, and after a lost connection the
  folder you were in is reopened first;
- the last local folder (`state.toml`). The window app opens there; the
  terminal app opens in the folder you start it from;
- hidden files and sort order, separately for the local and the remote panel
  (`state.toml`, under `[local_panel]` and `[remote_panel]`). A choice you make
  in the interface wins over `[panel]` in `config.toml` for that panel. To go
  back to the configured values, delete that table from `state.toml`.

A remembered folder that no longer exists is skipped without a message.

## Notices

Messages at the bottom of the screen disappear on their own: information after
4 seconds, warnings after 8, errors after 15. In the terminal app `Esc`
dismisses one early.
