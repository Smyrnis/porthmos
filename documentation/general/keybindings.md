# Keybindings

This page has two parts: the terminal app (`porthmos`), then the window app
(`porthmos-gui`), which has its own defaults and its own `[gui.keys]` table.

## Terminal app

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
| `F4` | Terminal in the active panel's folder: your shell on the local panel, `ssh` on the remote panel |
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

## Window app

The window app has one flat table of shortcuts, not one per screen. The
defaults are below. On macOS `Cmd` stands in for `Ctrl` where the table says
`Cmd/Ctrl`.

| Key | Action |
| --- | --- |
| `↑` `↓` / `k` `j` | Move |
| `PageUp` / `PageDown` / `Home` / `End` | Page up, page down, first entry, last entry |
| `Enter` | Open |
| `Backspace` | Go to the parent folder |
| `Tab` | Switch panel |
| `Space` | Select or deselect |
| `Cmd/Ctrl+A` | Select all |
| `Ctrl+H` | Show or hide hidden files |
| `Ctrl+S` | Change sort order |
| `Ctrl+R` | Refresh |
| `/` | Filter files |
| `Esc` | Clear the filter |
| `Ctrl+N` | Next connection tab |
| `F5` | Copy to the other panel |
| `F6` | Move to the other panel |
| `F7` | New folder |
| `F2` | Rename |
| `Delete` / `F8` | Delete |
| `e` / `F3` | Edit |
| `p` | Preview |
| `F4` | Open a terminal here: your shell on the local panel, `ssh` on the remote panel |
| `Ctrl+T` | Show or hide transfers |
| `Ctrl+F` | Search |
| `Ctrl+B` / `Ctrl+D` | Bookmarks / bookmark this folder |
| `Ctrl+Y` | Transfer history |
| `Ctrl+U` | Synchronize |
| `Ctrl+P` | Command palette |
| `Ctrl+,` | Settings |
| `Cmd/Ctrl+Q` | Quit |

"Previous connection tab" and "Check for updates" have no default key.
Shortcuts can be changed in the app's settings or in the `[gui.keys]` table of
`config.toml`, named after the action (`move_up`, `cycle_sort`, `palette`, and
so on). A value is one chord or a list of chords, such as `"Ctrl+G"`. A few
chords are reserved and cannot be assigned: `Alt+F4`, `Alt+Tab`, `F11`, any
`Ctrl+Shift` chord, and the Windows or Super key outside macOS.
