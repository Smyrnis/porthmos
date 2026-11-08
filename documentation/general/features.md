# Features

- **Dual-panel browser** — the local filesystem on one side, the active
  connection on the other. Several connections can stay open at once and you
  switch between them with `Ctrl+N` (the terminal app also has the number keys
  `1`–`9`). Entries show size, modification time, owner and link targets, and
  the remote panel shows free space.
- **Protocols** — SFTP and SCP (over SSH), FTP and FTPS, WebDAV, and S3 and
  S3-compatible stores (AWS, R2, MinIO, B2). SCP is for SSH servers without
  SFTP. SSH host keys go to `~/.ssh/known_hosts`. A new TLS certificate is
  shown to you and remembered in `known_certificates.toml` once you accept
  it; a certificate that has changed since is refused.
- **Go to a path** — type a folder path instead of clicking through. `~` is
  your home folder, a relative path starts from the folder you are in, and `..`
  goes up. In the terminal app press `Ctrl+'` on the file screen, type the path
  and press `Enter` (`Esc` cancels). In the window app click the empty part of
  the path bar, or click a folder in the breadcrumbs to jump to it.
- **Saved connections** — host, port, username, identity file, start folder
  and optional password, alongside the hosts read from `~/.ssh/config`.
  Connections can be grouped and tagged, and the list can be filtered. Saved
  passwords and secrets go to the system keyring when there is one; without a
  keyring they are kept only until Porthmos quits.
- **Background transfers** — copy and move, files or whole folders, queued
  and running in parallel. Transfers can be paused, reordered, cancelled and
  speed-limited. They are written to a `.part` file first. After a cancel, a
  dropped connection or a restart, copy the same file again and choose Resume
  to continue where it stopped; the queue itself is not restored after a
  restart. A conflict dialog offers overwrite, skip, rename, resume or cancel,
  for one file or for all of them.
- **Folder sync** — compare a local and a remote folder by time or size, one
  way or both ways, review the differences, and run only the ones you tick.
- **Edit remote files** — open a remote file in `$VISUAL`, `$EDITOR` or the
  editor set in `config.toml`, and upload it again when you save. A file that
  changed on the server in the meantime is never overwritten silently.
- **Preview** — see the start of a file without downloading it.
- **Search** — recursive filename search on either side, and a live filter
  (`/`) for the folder you are in. With `[search] preview` turned on (a
  checkbox in the window app's Settings), a pane below the results shows the
  start of the file under the cursor.
- **History** — a log of past transfers.
- **Bookmarks** — jump back to a local or remote folder on a given host.
- **Command palette** — `Ctrl+P` finds actions, hosts to connect or
  disconnect, and files in the open folders, by name.
- **Reopen last connections** — off by default. Turn it on with `[startup]
  reopen` in `config.toml` (the window app also has it in Settings). With
  `"ask"`, the empty remote panel and the command palette offer "Reopen last
  connections"; with `"auto"`, the connections open when the app starts. See
  [Configuration](configuration.md#reopening-connections).
- **Windows of the window app** — Settings, the connection manager and Search
  each open in their own window. Each window comes back at the size and
  position it had when you last quit (on Wayland, only the size). Settings
  also lets you rebind the shortcuts.
- **Terminal hand-off** — `F4` follows the panel you are in. On the local panel
  it opens your shell (`$SHELL`, or PowerShell on Windows) in that folder. On
  the remote panel of an SFTP or SCP connection it opens a full `ssh` session
  in the folder you are looking at. A saved password is passed to `ssh`
  automatically when `ssh` is OpenSSH 8.4 or newer (or when `sshpass` is
  installed); otherwise `ssh` asks for it. The terminal app suspends itself
  and returns when you exit; the window app opens a separate terminal, which
  you can choose with `[terminal] command` in `config.toml`, from the
  keyboard or from its two terminal buttons on the toolbar.
