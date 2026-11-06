# Updates and release channels

## Updates

Porthmos looks for a newer version in the background and, by default, downloads
and verifies it while you work. It never replaces the running program. When an
update is ready, `↑ 0.4.0 installs on quit` appears at the right end of the top
bar, and the new version is installed when you quit normally (`q` or `F10`);
Porthmos then prints `Porthmos updated to 0.4.0.` and the next start is the new
version. Closing the terminal window or killing the process installs nothing; the
update stays ready for the next normal quit.

Every download is checked against a signature made with a key built into the
program, so a compromised download host cannot push code. The only request made
is the one that asks for the latest version, and it carries just
`User-Agent: porthmos/<version>` (`porthmos-gui/<version>` for the window app).
A copy owned by a package manager (a system package, Homebrew, `cargo install`)
is never replaced; the badge then reads `↑ 0.4.0 available`.

Set `mode` in the `[update]` table of `config.toml`: `"auto"` (the default),
`"notify"` (show the badge, download and install nothing) or `"off"` (never
check). `interval_hours` sets how often it checks. "Check for updates" in the
command palette (`Ctrl+P`) checks right away and always says what it found.

From a script, `porthmos update` (or `porthmos-gui update`) checks, downloads
and installs the latest version without starting the interface, and
`porthmos update --check` only reports whether a newer version exists. Both
ignore `mode`.

The program is not code-signed, so macOS Gatekeeper and Windows SmartScreen may
warn the first time you run a downloaded copy.

## Release channels

Each program is released on two channels, and each has its own tags, version
and update feed.

| Channel | Built from | Terminal app tag | Window app tag | Good for |
| --- | --- | --- | --- | --- |
| Stable | `main` | `2026T.5.0` | `2026G.5.0` | Everyday use. |
| Preview | `development` | `2026T.5.0.dev.1` | `2026G.5.0.dev.1` | Trying new features early. May be less polished. |

A tag is the version with a letter after the year (`T` for the terminal app,
`G` for the window app) and the dash written as a dot, so the tag
`2026T.5.0.dev.1` is the terminal app at version `2026.5.0-dev.1`. The first
number is the year. A tag ending in something other than `.dev` or `.dev.N`,
such as `2026T.5.0.rc.1`, is a candidate: it is published as a pre-release but
is not offered through any feed. To give a release notes, create an annotated
tag (`git tag -a 2026T.5.0 -m "notes"`); the notes are the tag message.

All builds are made for Linux, macOS and Windows. A build only follows its own
program and channel: the preview terminal app is never offered the window app,
and a stable build is never offered a preview build. A preview build keeps
following the preview feed until you leave the channel by running `install.sh`
without `--preview`.

The feeds are rolling releases on GitHub that always hold the newest build and
its signed `latest.json`: `tui-stable`, `tui-preview`, `gui-stable` and
`gui-preview`. Preview downloads are signed with the same key as stable ones.
