# Installing and uninstalling

## From a release

The commands in this section are for Linux and macOS; see
[Platforms](#platforms) for Windows.

Porthmos is one program, `porthmos-gui`, which opens a window. On Linux and
macOS, the install script downloads the build for your machine, checks it
against its checksum, and puts it in `~/.local/share/porthmos` with a link in
`~/.local/bin`.

There is no stable release yet, so install the
[preview build](updates.md#release-channels) (add `--preview`):

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/gui-preview/install.sh | sh -s -- --preview
```

Before it changes anything, the script prints a plan: what it will download and
from where, the checksum check, which files it will create and which version it
replaces. It then asks `Continue? [y/N]`. The question is read from your
terminal, so it works through the pipe above. Answer `y` to go on; anything else
stops without changing anything. While it works it prints numbered steps and
ends with where Porthmos went, how to start it and how to remove it.

Two options change this. `--dry-run` prints the plan and stops: nothing is
downloaded, nothing is written and nothing is asked, so you can read what the
installer does first. `--yes` skips the question, which a script without a
terminal needs; the plan and the steps are still printed.

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/gui-preview/install.sh | sh -s -- --preview --dry-run
```

Older install commands that also pass `--gui` still work; the flag does
nothing. A terminal app (`porthmos`) was released earlier and is no longer
built; an old copy keeps working until you remove it, and `uninstall.sh`
removes it too.

On Linux, installing also adds a launcher and an icon, so
`Porthmos` appears in your applications menu. The programs go to
`$PORTHMOS_INSTALL_DIR` and the links to `$PORTHMOS_BIN_DIR` if you set them.

## From source

Requires a Rust toolchain supporting the 2024 edition (stable is fine).

```sh
cargo build --release
```

The binary is written to `target/release/porthmos-gui`.

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

<https://github.com/Smyrnis/porthmos/releases/download/gui-preview/porthmos-gui-setup.exe>

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

The program also comes as a `.zip` archive on the release page, for people who
do not want the installer; unpack it anywhere.

## Uninstalling

If you installed Porthmos with `install.sh`, run `uninstall.sh` from the same
release:

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/gui-preview/uninstall.sh | sh
```

It removes `porthmos-gui` (and the old terminal app, `porthmos`, if an earlier
install left it) from `~/.local/share/porthmos`, with the links in
`~/.local/bin`, the launcher and icon if the installer added them, and any previous version
an update left beside them, and keeps your settings, saved connections,
bookmarks, history and saved passwords, telling you where they are. To remove
those as well, add `--purge`. Like the installer, the script prints a plan of
what it will remove and what it will leave, and asks `Continue? [y/N]` every
time. `--dry-run` shows the plan and removes nothing, and `--yes` skips the
question (needed when the script runs without a terminal):

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/gui-preview/uninstall.sh | sh -s -- --purge
```

Add `--yes` only in a script that has no terminal: it skips the question that
guards your saved data.

On Windows, remove an installed copy from Apps and features. The uninstaller
deletes the program, the Start Menu and desktop shortcuts, and the `.old` and
`.porthmos.*` files an update leaves behind. It does not touch your `PATH`,
file associations, settings, saved connections or saved passwords. A copy
unpacked from a `.zip` is removed by deleting its folder.

`--purge` also asks the system keyring (`secret-tool` on Linux, the keychain on
macOS) to forget the passwords Porthmos saved. It never touches
`~/.ssh/known_hosts`, which other programs use. A copy installed any other way
(a package manager, Homebrew, `cargo install`) is removed with that tool.
