# Installing and uninstalling

## From a release

The commands in this section are for Linux and macOS; see
[Platforms](#platforms) for Windows.

Porthmos comes as two programs: `porthmos`, which runs in the terminal, and
`porthmos-gui`, which opens a window. They are released separately and can be
installed side by side. On Linux and macOS, the install script downloads the
build for your machine, checks it against its checksum, and puts it in
`~/.local/share/porthmos` with a link in `~/.local/bin`.

The terminal app:

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/tui-stable/install.sh | sh
```

The window app (add `--gui`):

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/gui-stable/install.sh | sh -s -- --gui
```

To try the newest features before they are released, install the
[preview build](updates.md#release-channels) instead (add `--preview`):

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/tui-preview/install.sh | sh -s -- --preview
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/gui-preview/install.sh | sh -s -- --gui --preview
```

Run the script again without `--preview` to go back to the stable build.
Installing one program never touches the other.

On Linux, installing the window app also adds a launcher and an icon, so
`Porthmos` appears in your applications menu. The programs go to
`$PORTHMOS_INSTALL_DIR` and the links to `$PORTHMOS_BIN_DIR` if you set them.

## From source

Requires a Rust toolchain supporting the 2024 edition (stable is fine).

```sh
cargo build --release
```

The binary is written to `target/release/porthmos`.

To build the window app instead, run `cargo build --release -p porthmos-gui`;
the binary is `target/release/porthmos-gui`.

The F4 terminal hand-off passes a saved profile password to `ssh` by itself
when `ssh` is OpenSSH 8.4 or newer; no extra program is needed. With an older
`ssh`, install [`sshpass`](https://linux.die.net/man/1/sshpass) to get the same
on Linux and macOS, or `ssh` asks for the password again.

## Platforms

Releases are built for Linux (x86_64 and aarch64), macOS (Apple silicon and
Intel) and Windows (x86_64). The install and uninstall scripts below are for
Linux and macOS. Settings and state live in different folders on Windows; see
[Configuration](configuration.md).

### Windows

The window app has an installer, `porthmos-gui-setup.exe`:

- Stable: <https://github.com/Smyrnis/porthmos/releases/download/gui-stable/porthmos-gui-setup.exe>
- Preview: <https://github.com/Smyrnis/porthmos/releases/download/gui-preview/porthmos-gui-setup.exe>

Each release also carries a copy with the version in its name,
`porthmos-gui-<version>-setup.exe`. The wizard shows a welcome page, the
licence, the install folder, an optional desktop shortcut, a summary and a
finish page with a "Start Porthmos" box.

It installs for the current user only, into
`%LOCALAPPDATA%\Programs\Porthmos`, and asks for no administrator rights. It
adds a Start Menu entry and an entry in Apps and features. Running a newer
installer upgrades in place, and a running Porthmos is closed first. Keep the
default folder: the updater leaves programs under Program Files alone because
it treats them as managed by something else, so a copy installed there stops
updating itself.

The terminal app has no installer. Download the `.zip` archive of the program
you want from the release page and unpack it anywhere; the window app comes as
a `.zip` too, for people who do not want the installer.

## Uninstalling

If you installed Porthmos with `install.sh`, run `uninstall.sh` from the same
release:

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/tui-stable/uninstall.sh | sh
```

It removes whichever of `porthmos` and `porthmos-gui` is installed from
`~/.local/share/porthmos`, with their links in `~/.local/bin`, the window
app's launcher and icon if the installer added them, and any previous version
an update left beside them, and keeps your settings, saved connections,
bookmarks, history and saved passwords, telling you where they are. To remove
those as well, add `--purge`; it lists what it will delete and asks first, or
skips the question with `--yes` (needed when the script is piped, as above):

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/tui-stable/uninstall.sh | sh -s -- --purge --yes
```

On Windows, remove an installed copy from Apps and features. The uninstaller
deletes the program, the Start Menu and desktop shortcuts, and the `.old` and
`.porthmos.*` files an update leaves behind. It does not touch your `PATH`,
file associations, settings, saved connections or saved passwords. A copy
unpacked from a `.zip` is removed by deleting its folder.

`--purge` also asks the system keyring (`secret-tool` on Linux, the keychain on
macOS) to forget the passwords Porthmos saved. It never touches
`~/.ssh/known_hosts`, which other programs use. A copy installed any other way
(a package manager, Homebrew, `cargo install`) is removed with that tool.
