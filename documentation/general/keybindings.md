# Keybindings

The defaults below can be changed in the `[gui.keys]` table of `config.toml`.

The window app has one flat table of shortcuts, not one per screen. Settings,
the connection manager and Search open in their own windows. To go to a typed
path, click the empty part of a panel's path bar. The defaults are below. On macOS `Cmd` stands in for `Ctrl` where the table says
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
| `Ctrl+S` | Change sort order: name, size, modified, type, each ascending then descending |
| `Ctrl+R` | Refresh |
| `/` | Filter files |
| `Esc` | Clear the filter |
| `Ctrl+N` | Next connection tab |
| `F5` | Copy to the other panel |
| `F6` | Move to the other panel |
| `F7` | New folder |
| `F2` | Rename |
| `Shift+P` | Permissions |
| `Alt+Enter` | Properties: size and SHA-256 checksum |
| `Delete` / `F8` | Delete |
| `e` / `F3` | Edit |
| `p` | Preview |
| `F4` | Open a terminal here: your shell on the local panel, `ssh` on the remote panel |
| `Ctrl+T` | Show or hide transfers |
| `Ctrl+F` | Search (in its window, `Enter` raises the main window, `Shift+Enter` keeps the focus in Search) |
| `Ctrl+B` / `Ctrl+D` | Bookmarks / bookmark this folder |
| `Ctrl+Y` | Transfer history |
| `Ctrl+U` | Synchronize |
| `=` | Compare the two folders and mark the differences (press again to turn off) |
| `Ctrl+P` | Command palette (`>` actions only, `@` hosts only, `#` files) |
| `Ctrl+,` | Settings |
| `Cmd/Ctrl+Q` | Quit |

"Previous connection tab" and "Check for updates" have no default key.
Shortcuts can be changed in the app's settings or in the `[gui.keys]` table of
`config.toml`, named after the action (`move_up`, `cycle_sort`, `palette`, and
so on). A value is one chord or a list of chords, such as `"Ctrl+G"`. A few
chords are reserved and cannot be assigned: `Alt+F4`, `Alt+Tab`, `F11`, any
`Ctrl+Shift` chord, and the Windows or Super key outside macOS.
