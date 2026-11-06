# Features

- **Dual-panel browser** — the local filesystem on one side, the active
  connection on the other. Several connections can stay open at once and you
  switch between them with `Ctrl+N` or the number keys. Entries show size,
  modification time, owner and link targets, and the remote panel shows free
  space.
- **Protocols** — SFTP and SCP (over SSH), FTP and FTPS, WebDAV, and S3 and
  S3-compatible stores (AWS, R2, MinIO, B2). SCP is for SSH servers without
  SFTP. SSH host keys go to `~/.ssh/known_hosts`. A new TLS certificate is
  shown to you and remembered in `known_certificates.toml` once you accept
  it; a certificate that has changed since is refused.
- **Saved connections** — host, port, username, identity file, start folder
  and optional password, alongside the hosts read from `~/.ssh/config`.
  Connections can be grouped and tagged, and the list can be filtered. Saved
  passwords and secrets go to the system keyring when there is one; without a
  keyring they are kept only until Porthmos quits.
- **Background transfers** — copy and move, files or whole folders, queued
  and running in parallel. Transfers can be paused, reordered, cancelled and
  speed-limited. They are written to a `.part` file first and resume after a
  cancel, a dropped connection or a restart. A conflict dialog offers
  overwrite, skip, rename or cancel, for one file or for all of them.
- **Folder sync** — compare a local and a remote folder by time or size, one
  way or both ways, review the differences, and run only the ones you tick.
- **Edit remote files** — open a remote file in `$VISUAL`, `$EDITOR` or the
  editor set in `config.toml`, and upload it again when you save. A file that
  changed on the server in the meantime is never overwritten silently.
- **Preview** — see the start of a file without downloading it.
- **Search** — recursive filename search on either side, and a live filter
  (`/`) for the folder you are in.
- **History** — a log of past transfers.
- **Bookmarks** — jump back to a local or remote folder on a given host.
- **Command palette** — `Ctrl+P` finds actions, hosts to connect or
  disconnect, and files in the open folders, by name.
- **Terminal hand-off** — `F4` follows the panel you are in. On the local panel
  it opens your shell (`$SHELL`, or PowerShell on Windows) in that folder. On
  the remote panel of an SFTP or SCP connection it opens a full `ssh` session
  in the folder you are looking at. The terminal app suspends itself and
  returns when you exit; the window app opens a separate terminal, which you
  can choose with `[terminal] command` in `config.toml`.
