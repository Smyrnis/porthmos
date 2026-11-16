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
- **Import connections** — bring saved sites in from FileZilla
  (`sitemanager.xml`), WinSCP (`WinSCP.ini`) or PuTTY (its saved sessions, or a
  `.reg` export). You see a table of what was found, tick the rows you want, and
  rows whose name already exists are marked and left alone. Passwords are never
  imported, so you type each one on first connect. In the terminal app choose
  "Import connections" in the command palette (`Ctrl+P`) and press `Ctrl+T` to
  complete the path; in the window app press Import… in the connection manager,
  where Browse… opens a file chooser.
- **Jump hosts** — an SFTP or SCP connection can reach a server that is only
  reachable through one or more bastion hosts. Fill in the connection's "Jump
  host" (`[user@]host[:port]`, comma-separated for several hops, or `none` to
  switch it off), or set `ProxyJump` in `~/.ssh/config`. A saved connection with
  no jump host of its own uses the one of the `~/.ssh/config` entry with the same
  name, host and port. Every hop asks for its host key and password like a normal
  connection, and the terminal button passes the same chain to `ssh -J`.
  `ProxyCommand` and HTTP or SOCKS proxies are not supported.
- **Reconnect** — off by default. With `[connection] reconnect = true` (also in
  the window app's Settings) a dropped connection is retried after 2, 5, 10, 20
  and 40 seconds and the remote panel returns to the folder it was in. It stops
  at once if a password is refused.
- **Background transfers** — copy and move, files or whole folders, queued
  and running in parallel. Transfers can be paused, reordered, cancelled and
  speed-limited. They are written to a `.part` file first. After a cancel or a
  dropped connection, copy the same file again and choose Resume to continue
  where it stopped. Unfinished transfers are saved (every few seconds while data
  moves) and come back after a restart or a crash, stopped and marked "Porthmos
  was closed before this finished"; retry them to continue from the saved byte
  count. A restored move also removes the source folders it emptied. A conflict dialog offers overwrite, skip, rename, resume or cancel,
  for one file or for all of them. Copies between this computer and an SFTP
  or SCP connection keep the file permissions (for example a script stays
  executable); other protocols have no permissions to keep, and a warning says
  so.
- **Folder sync** — compare a local and a remote folder by time or size, one
  way or both ways, review the differences, and run only the ones you tick.
  A one-way sync can also mirror: files and folders that are missing from the
  source are listed as `Delete` rows, none ticked by default, and a confirmation
  names how many files and folders will go. A mirror of an empty source is
  skipped.
  A folder that exists on one side only and is empty is listed as "new empty
  folder" and is created too; a chain of empty folders appears as its innermost
  folder.
- **Edit remote files** — open a remote file in `$VISUAL`, `$EDITOR` or the
  editor set in `config.toml`, and upload it again when you save. A file that
  changed on the server in the meantime is never overwritten silently. The
  question offers "Show differences", which lists what the server's copy and
  yours differ by (files up to `[edit] compare_limit_mib`, 1 MiB by default).
- **Preview** — see the start of a file without downloading it.
- **Properties** — `i` in the terminal app, Alt+Enter or the context menu in
  the window app. It measures the size of the chosen files and folders (with a
  progress count you can cancel) and calculates a file's SHA-256 checksum, which
  you can compare with a digest you paste.
- **Permissions** — `P` on SFTP, SCP and the local panel. The terminal app takes
  an octal mode; add `-R` (`755 -R`) to change everything inside the chosen
  folders too. The window app has read, write and execute boxes and an "Apply to
  everything inside folders" box. Links are not followed. WebDAV and S3 have no
  modes.
- **Compare folders** — `=` (or "Compare folders" in the palette) marks, in both
  panels, what differs between the two folders shown: only on this side, newer,
  older, a different size, or a file against a folder. It compares names, then
  modification time (2 seconds apart is equal), then size, and does not look
  inside subfolders. Press `=` again to turn it off.
- **Sorting** — by name, size, modification time or type (the file extension),
  ascending or descending. `Ctrl+S` cycles through them; in the window app the
  column headers are clickable too.
- **Search** — recursive filename search on either side, and a live filter
  (`/`) for the folder you are in. With `[search] preview` turned on (a
  checkbox in the window app's Settings), a pane below the results shows the
  start of the file under the cursor.
- **History** — a log of past transfers.
- **Bookmarks** — jump back to a local or remote folder on a given host.
- **Command palette** — `Ctrl+P` finds actions and hosts to connect or
  disconnect by name. Start the query with `>` to see only actions, `@` for
  only hosts, or `#` to search the files in the open folders.
- **Reopen last connections** — off by default. Turn it on with `[startup]
  reopen` in `config.toml` (the window app also has it in Settings). With
  `"ask"`, the empty remote panel and the command palette offer "Reopen last
  connections"; with `"auto"`, the connections open when the app starts, and
  the tab that was active comes back active. See
  [Configuration](configuration.md#reopening-connections).
- **Windows of the window app** — Settings, the connection manager and Search
  each open in their own window. Each window comes back at the size and
  position it had when you last quit, maximized if it was (on Wayland, only the
  size). A new connection makes its remote panel the active one. Settings
  also lets you rebind the shortcuts.
- **Language** — every menu, dialog and message of both apps is available in
  English and Greek (Ελληνικά). The language comes from `[general] language`
  in `config.toml`, or from `PORTHMOS_LANG`, `LC_ALL` or `LANG` when that is
  `"auto"`, and anything unknown falls back to English. The window app also
  has a Language choice in Settings that applies at once. See
  [Configuration](configuration.md#language).
- **Terminal hand-off** — `F4` follows the panel you are in. On the local panel
  it opens your shell (`$SHELL`, or PowerShell on Windows) in that folder. On
  the remote panel of an SFTP or SCP connection it opens a full `ssh` session
  in the folder you are looking at. A saved password is passed to `ssh`
  automatically when `ssh` is OpenSSH 8.4 or newer (or when `sshpass` is
  installed); otherwise `ssh` asks for it. The terminal app suspends itself
  and returns when you exit; the window app opens a separate terminal, which
  you can choose with `[terminal] command` in `config.toml`, from the
  keyboard or from its two terminal buttons on the toolbar.
