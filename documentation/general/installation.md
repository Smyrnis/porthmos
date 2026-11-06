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

## From source

Requires a Rust toolchain supporting the 2024 edition (stable is fine).

```sh
cargo build --release
```

The binary is written to `target/release/porthmos`.

To build the window app instead, run `cargo build --release -p porthmos-gui`;
the binary is `target/release/porthmos-gui`.

Optional on Linux and macOS: install
[`sshpass`](https://linux.die.net/man/1/sshpass) if you want the F4 terminal
hand-off to use a saved profile password automatically, instead of prompting
for it again. On Windows the hand-off always prompts again.

## Platforms

Releases are built for Linux (x86_64 and aarch64), macOS (Apple silicon and
Intel) and Windows (x86_64). The install and uninstall scripts below are for
Linux and macOS. On Windows there is no install script: the release builds are
`.zip` archives, so unpack the one for the program you want. Settings and state
live in different folders there; see [Configuration](configuration.md).

## Uninstalling

If you installed Porthmos with `install.sh`, run `uninstall.sh` from the same
release:

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/tui-stable/uninstall.sh | sh
```

It removes whichever of `porthmos` and `porthmos-gui` is installed from
`~/.local/share/porthmos`, with their links in `~/.local/bin` and any previous
version an update left beside them, and keeps your settings, saved connections,
bookmarks, history and saved passwords, telling you where they are. To remove
those as well, add `--purge`; it lists what it will delete and asks first, or
skips the question with `--yes` (needed when the script is piped, as above):

```sh
curl -fsSL https://github.com/Smyrnis/porthmos/releases/download/tui-stable/uninstall.sh | sh -s -- --purge --yes
```

`--purge` also asks the system keyring (`secret-tool` on Linux, the keychain on
macOS) to forget the passwords Porthmos saved. It never touches
`~/.ssh/known_hosts`, which other programs use. A copy installed any other way
(a package manager, Homebrew, `cargo install`) is removed with that tool.
