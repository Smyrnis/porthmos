# Configuration

Settings, saved connections, and bookmarks live under
`$XDG_CONFIG_HOME/porthmos` (or `~/.config/porthmos` if unset) on Linux and
macOS, and under `%APPDATA%\porthmos` on Windows:

- `config.toml` — panel, transfer, edit, terminal, update, search, startup,
  connection, key-binding and theme settings. See
  [`config/config.example.toml`](../../config/config.example.toml) for every
  option with its default.
- `connections.toml` — saved connection profiles.
- `bookmarks.toml` — saved bookmarks.
- `known_certificates.toml` — TLS certificates you chose to trust.
- `theme.toml` — optional colours for both apps (a base of `dark`,
  `high-contrast` or `light`, and any colour you want to change). See the end
  of [`config/config.example.toml`](../../config/config.example.toml). Changes
  are read when the app starts.

Transfer history and other state live under `$XDG_STATE_HOME/porthmos`
(or `~/.local/state/porthmos`) on Linux and macOS, and under
`%LOCALAPPDATA%\porthmos` on Windows. That folder holds `state.toml`,
`last_used.toml`, `history.toml`, `unfinished-transfers.toml` (transfers that
were not finished when you quit), `gui-windows.toml` (the window app's window
sizes and positions) and the log.

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
and Windows. The Settings screen has a Terminal row that applies at once.

## What Porthmos remembers

Between runs Porthmos keeps, in the state folder:

- the last folder of each saved connection (`last_used.toml`, next to its
  last-used time). A connection with a start folder set still opens there; the
  remembered folder is used when none is set, and after a lost connection the
  folder you were in is reopened first;
- the last local folder (`state.toml`), where the app opens;
- hidden files and sort order, separately for the local and the remote panel
  (`state.toml`, under `[local_panel]` and `[remote_panel]`). A choice you make
  in the interface wins over `[panel]` in `config.toml` for that panel. To go
  back to the configured values, delete that table from `state.toml`.

- the size and position of the window app's windows — main, Settings,
  connection manager and Search (`gui-windows.toml`), written when the app
  quits normally, and whether each was maximized (a maximized window keeps its
  normal size for when you restore it). On Wayland only the size is restored,
  because the compositor does not tell the app where a window is. Porthmos does
  not check that a saved position is still on a screen; your window manager
  normally moves a window that would open off screen;
- the transfers that were not finished (`unfinished-transfers.toml`): written
  whenever the set of unfinished transfers changes and every 5 seconds while
  data moves, and read at the next start. A transfer that was running comes
  back stopped and resumes from the saved byte count when you retry it;
- the connections that were open, and which one was active, only when
  `[startup] reopen` is not `"off"` (`state.toml`, keys `reopen` and
  `reopen_active`); see below.

A remembered folder that no longer exists is skipped without a message.

## Reopening connections

`[startup] reopen` in `config.toml` chooses what happens to the saved
connections that were open when you last quit:

- `"off"` (the default) — nothing is remembered and nothing is offered.
- `"ask"` — the empty remote panel shows a button naming them, and the command palette lists "Reopen last
  connections". Nothing connects until you choose it.
- `"auto"` — they are opened one after another when the app starts.

Connections open one at a time, so passwords, key passphrases and host-key
questions come one by one as usual, and a connection that fails does not stop
the ones after it. A connection whose password is saved in the keyring opens
without a prompt, and each one returns to the folder it was last in. Only
saved connections are remembered, in the order you opened them. Changing the
setting in Settings applies at once.

## Proxies

An SFTP or SCP connection keeps its proxy settings in `connections.toml` with
the other options of the connection: `proxy` (`http://host:port`,
`socks5://host:port` or `socks4://host:port`; the port defaults to 8080 for HTTP
and 1080 for SOCKS), `proxy_user`, `proxy_command` and `proxy_jump`. `none` in
`proxy` means no proxy, `none` in `proxy_command` stops the connection from
using the `ProxyCommand` of its `~/.ssh/config` entry, and `none` in
`proxy_jump` stops it from using that entry's `ProxyJump`. The proxy
password is never written to `connections.toml`: it goes to the system keyring
under the connection's name, or is kept until Porthmos quits when there is no
keyring.

A saved connection with no proxy and no jump host of its own uses the
`ProxyCommand` of the `~/.ssh/config` entry that has the same name, host and
port. When a connection has a jump host, that wins over a `ProxyCommand` from
`~/.ssh/config`. Setting `proxy` and `proxy_command` together is an error, and
so is a proxy user or password without a proxy.

A proxy command runs in your shell (`sh -c`, or `cmd /C` on Windows). Porthmos
gives up when the command sends nothing for 30 seconds and shows what it
printed to standard error. A value filled into the command (`%h`, `%r`, and so
on) is refused when it holds anything but letters, digits and a few safe
punctuation marks, or starts with `-`.

The terminal button (`F4` on the remote panel) cannot take a proxy login or a
proxy together with a jump host, so it does not connect then. For an HTTP or
SOCKS proxy without a login it runs `ssh` with `ProxyCommand=nc -X ... -x ...`,
which needs the OpenBSD `nc` (`netcat-openbsd`); on Linux and macOS Porthmos
checks first and says so when `nc` is another version.

## Search limits

`[search] max_depth` (1 to 64, default 16) is how many folder levels below the
starting folder a search reads, and `max_results` (10 to 10000, default 1000) is
how many matches it keeps before it stops and says the list was cut short.
Folders such as `.git` and `node_modules` are searched too.

## Sorting

`[panel] sort_key` is `"name"`, `"size"`, `"modified"` or `"type"` (the file
extension) and `sort_order` is `"ascending"` or `"descending"`. A sort you
choose in the interface is remembered for that panel and wins over the setting.

## Language

The text of both apps comes from one language file per language inside
`porthmos-core`. English (`en`) and Greek (`el`) exist today.

`[general] language` is `"auto"` (the default), `"en"` or `"el"`. With `"auto"`
the language comes from the `PORTHMOS_LANG` environment variable, then
`LC_ALL`, then `LANG` (`el_GR.UTF-8` and `el` style values are both
understood); a language that has no file falls back to English. An unknown
value in `config.toml` is reported as a warning and treated as `"auto"`.

The window app has a Language section in Settings > General (Automatic,
English, Ελληνικά) that applies at once and writes the setting. The terminal
app reads the setting when it starts, so a change needs a restart.

Config and theme warnings, error causes from the operating system and key
names stay in English.

## Notices

Messages at the bottom of the screen disappear on their own: information after
4 seconds, warnings after 8, errors after 15.
