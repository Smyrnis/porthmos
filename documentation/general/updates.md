# Updates and release channels

## Updates

Porthmos looks for a newer version in the background and, by default, downloads
and verifies it while you work. It never replaces the running program. When an
update is ready, `↑ 2026.6.0 installs on quit` appears at the right end of the
toolbar, and the new version is installed when you quit normally (closing the
window or `Cmd/Ctrl+Q`). Porthmos then prints
`Porthmos updated to 2026.6.0.` to the terminal it was started from, and the
next start is the new version. Closing the terminal window or killing the
process installs nothing; the update stays ready for the next normal quit.

Every download is checked against a signature made with a key built into the
program, so a compromised download host cannot push code. The only request made
is the one that asks for the latest version, and it carries just
`User-Agent: porthmos-gui/<version>`.
A copy owned by a package manager (a system package, Homebrew, `cargo install`)
is never replaced; the badge then reads `↑ 2026.6.0 available`.

Set `mode` in the `[update]` table of `config.toml`: `"auto"` (the default),
`"notify"` (show the badge, download and install nothing) or `"off"` (never
check). `interval_hours` sets how often it checks. "Check for updates" in the
command palette (`Ctrl+P`) checks right away and always says what it found.

From a script, `porthmos-gui update` checks, downloads
and installs the latest version without starting the interface, and
`porthmos-gui update --check` only reports whether a newer version exists. Both
ignore `mode`.

The program is not code-signed, so macOS Gatekeeper and Windows SmartScreen may
warn the first time you run a downloaded copy, and for the Windows installer.
The installer is only for the first install: updates download the program
archive, never the installer, and replace the installed program in place.

## Release channels

Porthmos is released on two channels, each with its own tags, version and
update feed. Only the preview channel is published for now; there is no stable
release yet.

| Channel | Built from | Tag | Good for |
| --- | --- | --- | --- |
| Stable | `main` | `2026.5.0` | Everyday use (not published yet). |
| Preview | `development` | `2026.5.0-dev` or `2026.5.0-dev.1` | Trying new features early. May be less polished. |

A tag is the version itself, so the tag `2026.5.0-dev.1` is version
`2026.5.0-dev.1`. The first number is the year. A suffix other than `-dev` or
`-dev.N`, such as `2026.5.0-rc.1`, makes a candidate: it is published as a
pre-release but is not offered through any feed. To give a release notes,
create an annotated tag (`git tag -a 2026.5.0 -m "notes"`); the notes are the
tag message. Older tags with a letter after the year (`2026G.5.0.dev.5`, and
`2026T.…` for the terminal app, which is no longer released) belong to the old
format; they stay as history and new ones are refused.

All builds are made for Linux, macOS and Windows. A build only follows its own
channel: a stable build is never offered a preview build. A preview build keeps
following the preview feed until you leave the channel by running `install.sh`
without `--preview`.

The feeds are rolling releases on GitHub that always hold the newest build and
its signed `latest.json`: `gui-stable` and `gui-preview`. Preview downloads are signed with the same key as stable ones.
