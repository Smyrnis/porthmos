#!/bin/sh
set -eu

RELEASES="https://github.com/Smyrnis/porthmos/releases/download"

say() {
  printf '%s\n' "$*"
}

step_open=0

fail() {
  if [ "$step_open" -eq 1 ]; then
    say "failed"
    step_open=0
  fi
  printf 'porthmos install: %s\n' "$*" >&2
  exit 1
}

step() {
  step_number=$((step_number + 1))
  step_open=1
  label="[$step_number/$step_total] $1 "
  while [ "${#label}" -lt 32 ]; do
    label="$label."
  done
  printf '%s ' "$label"
}

step_done() {
  step_open=0
  say "$1"
}

usage() {
  cat << 'USAGE'
Usage: install.sh [--preview] [--dry-run] [--yes] [--gui]

Installs the Porthmos window app (porthmos-gui) into $PORTHMOS_INSTALL_DIR
(default ~/.local/share/porthmos) and links it from $PORTHMOS_BIN_DIR
(default ~/.local/bin).

  --gui       accepted and ignored: the window app is the only app. It stays
              so that install commands written before the terminal app was
              retired keep working.
  --preview   install the preview build instead of the stable one. Preview
              builds come from the development branch, get new features
              first, and may be less polished. They update from the preview
              feed. Run this script again without --preview to go back to
              the stable build.
  --dry-run   print what the installer would do and change nothing: no
              download, no file written, no question.
  --yes, -y   do not ask before installing. Needed when the script is run
              without a terminal. The plan is still printed.
  --help      show this text.

Before it changes anything the installer prints a plan and asks whether to
continue. The question is read from the terminal, so it works through
curl | sh as well.
USAGE
}

release_base() {
  case $2 in
    preview) printf '%s\n' "${PORTHMOS_PREVIEW_BASE:-$RELEASES/$1-preview}" ;;
    *) printf '%s\n' "${PORTHMOS_RELEASE_BASE:-$RELEASES/$1-stable}" ;;
  esac
}

detect_target() {
  case "$1:$2" in
    Linux:x86_64) echo x86_64-unknown-linux-gnu ;;
    Linux:aarch64 | Linux:arm64) echo aarch64-unknown-linux-gnu ;;
    Darwin:x86_64) echo x86_64-apple-darwin ;;
    Darwin:arm64) echo aarch64-apple-darwin ;;
    *)
      printf 'unsupported platform: %s %s\n' "$1" "$2" >&2
      return 1
      ;;
  esac
}

download() {
  curl -fsSL "$@"
}

verify_checksum() {
  directory=$1
  sums=$2
  if command -v sha256sum > /dev/null 2>&1; then
    (cd "$directory" && sha256sum -c "$sums" > /dev/null 2>&1)
  elif command -v shasum > /dev/null 2>&1; then
    (cd "$directory" && shasum -a 256 -c "$sums" > /dev/null 2>&1)
  else
    fail "sha256sum or shasum is required to check the download"
  fi
}

download_release_files() {
  for name in "$archive" "$archive.sha256"; do
    code=0
    download "$@" -o "$temp/$name" "$base/$name" || code=$?
    [ "$code" -eq 0 ] && continue
    if [ "$code" -eq 22 ] && [ "$channel" = stable ]; then
      fail "couldn't download $name from $base. If no stable release has been published yet, run again with --preview for the preview build."
    fi
    fail "couldn't download $name from $base"
  done
}

fetch_verified() {
  step "Downloading"
  download_release_files "$@"
  step_done "done"
  step "Checking SHA-256"
  attempt=1
  until verify_checksum "$temp" "$archive.sha256"; do
    [ "$attempt" -lt 2 ] || fail "the download does not match its checksum; nothing was installed"
    printf 'no match, fetching again ... '
    attempt=$((attempt + 1))
    sleep "${PORTHMOS_RETRY_DELAY:-3}"
    download_release_files "$@"
  done
  step_done "matches"
}

data_home() {
  case ${XDG_DATA_HOME:-} in
    /*) printf '%s\n' "$XDG_DATA_HOME" ;;
    *) printf '%s\n' "$HOME/.local/share" ;;
  esac
}

write_launcher() {
  cat << LAUNCHER
[Desktop Entry]
Type=Application
Name=Porthmos
Comment=Connect to servers and move files
Exec="$1"
Icon=$2
Terminal=false
Categories=Network;FileTransfer;
Keywords=sftp;ftp;ssh;scp;s3;webdav;
StartupWMClass=$2
X-Porthmos-Installer=install.sh
LAUNCHER
}

install_launcher() {
  program=$1
  executable=$2
  icon=$3
  if [ ! -f "$icon" ]; then
    say "the release has no icon"
    return 1
  fi
  case $executable in
    *'"'* | *'$'* | *'`'* | *\\* | *'%'*)
      say "the install path has a character a launcher can't hold"
      return 1
      ;;
  esac
  data=$(data_home)
  icon_dir=$data/icons/hicolor/scalable/apps
  app_dir=$data/applications
  pending=$app_dir/.$program.desktop.$$
  if ! { mkdir -p "$icon_dir" "$app_dir" && cp "$icon" "$icon_dir/$program.svg" &&
    write_launcher "$executable" "$program" > "$pending" && mv -f "$pending" "$app_dir/$program.desktop"; } 2> /dev/null; then
    rm -f "$pending"
    say "it could not be written; $program still runs from the command line"
    return 1
  fi
  if command -v update-desktop-database > /dev/null 2>&1; then
    update-desktop-database "$app_dir" > /dev/null 2>&1 || true
  fi
  return 0
}

tilde() {
  case $1 in
    "$HOME"/*) printf '~/%s\n' "${1#"$HOME"/}" ;;
    *) printf '%s\n' "$1" ;;
  esac
}

existing_version() {
  [ -f "$1" ] || return 0
  found=$("$1" --version 2> /dev/null < /dev/null || true)
  found=${found#"$2 "}
  if [ -n "$found" ]; then
    printf 'replaces %s\n' "$found"
  else
    printf 'replaces the existing copy\n'
  fi
}

print_plan() {
  if [ "$channel" = preview ]; then
    say "Porthmos installer (preview build, from the development channel; it may be less stable than a release)"
  else
    say "Porthmos installer (stable build)"
  fi
  say ""
  say "This will:"
  say "  download  $base/$archive"
  say "  check     its SHA-256 against the $archive.sha256 file in the same release"
  say "  install   $(tilde "$install_dir/$program")   (${existing:-new install})"
  say "  link      $(tilde "$bin_dir/$program")"
  if [ "$(uname -s)" = Linux ]; then
    say "  add       a launcher and icon in $(tilde "$(data_home)/applications"), if the release includes one"
  fi
  say "It needs no root. Your settings and saved connections are not touched."
  say ""
}

agree() {
  tty=${PORTHMOS_INSTALL_TTY:-/dev/tty}
  printf 'Continue? [y/N] ' >&2
  answer=$(head -n 1 2> /dev/null < "$tty") || fail "can't ask for confirmation without a terminal; add --yes to confirm"
  case $answer in
    [yY] | [yY][eE][sS]) return 0 ;;
    *) return 1 ;;
  esac
}

main() {
  channel=stable
  dry_run=0
  yes=0
  product=gui
  program=porthmos-gui
  for argument in "$@"; do
    case $argument in
      --preview) channel=preview ;;
      --gui) ;;
      --dry-run) dry_run=1 ;;
      --yes | -y) yes=1 ;;
      --help | -h)
        usage
        exit 0
        ;;
      *)
        usage >&2
        exit 2
        ;;
    esac
  done

  base=$(release_base "$product" "$channel")
  [ -n "${HOME:-}" ] || fail "HOME is not set"
  install_dir=${PORTHMOS_INSTALL_DIR:-$HOME/.local/share/porthmos}
  bin_dir=${PORTHMOS_BIN_DIR:-$HOME/.local/bin}

  case "$base" in
    https://*) set -- --proto '=https' --proto-redir '=https' --tlsv1.2 ;;
    file://*) set -- --proto '=file' ;;
    *) fail "the release location must start with https://" ;;
  esac
  if [ "$dry_run" -eq 0 ]; then
    command -v curl > /dev/null 2>&1 || fail "curl is required"
  fi

  if [ -n "${PORTHMOS_TARGET:-}" ]; then
    target=$PORTHMOS_TARGET
  else
    target=$(detect_target "$(uname -s)" "$(uname -m)") || exit 1
  fi

  archive=$program-$target.tar.gz
  existing=$(existing_version "$install_dir/$program" "$program")
  print_plan
  if [ "$dry_run" -eq 1 ]; then
    say "Dry run: nothing was downloaded or changed."
    exit 0
  fi
  if [ "$yes" -eq 0 ] && ! agree; then
    say "Stopped; nothing was changed."
    exit 1
  fi

  temp=$(mktemp -d "${TMPDIR:-/tmp}/porthmos-install.XXXXXX") || fail "couldn't create a temporary folder"
  trap 'rm -rf "$temp"' EXIT
  trap 'exit 1' HUP INT TERM

  step_number=0
  step_total=3
  [ "$(uname -s)" = Linux ] && step_total=4

  fetch_verified "$@"

  step "Installing"
  mkdir "$temp/extract"
  tar -xzf "$temp/$archive" -C "$temp/extract" "$program" || fail "the archive does not contain the $program program"
  tar -xzf "$temp/$archive" -C "$temp/extract" "$program.svg" > /dev/null 2>&1 || true
  mkdir -p "$install_dir" "$bin_dir" || fail "couldn't create $install_dir or $bin_dir"
  staged=$install_dir/.porthmos.new.$$
  if ! { cp "$temp/extract/$program" "$staged" && chmod 755 "$staged" && mv -f "$staged" "$install_dir/$program"; }; then
    rm -f "$staged"
    fail "couldn't install into $install_dir"
  fi
  ln -sf "$install_dir/$program" "$bin_dir/$program" || fail "couldn't link $bin_dir/$program"
  step_done "done"

  launcher_added=0
  if [ "$step_total" -eq 4 ]; then
    step "Adding the launcher"
    if reason=$(install_launcher "$program" "$install_dir/$program" "$temp/extract/$program.svg"); then
      launcher_added=1
      step_done "done"
    else
      step_done "skipped ($reason)"
    fi
  fi

  version=$("$install_dir/$program" --version 2> /dev/null || true)
  say ""
  say "Installed ${version:-$program}"
  say "  where:     $(tilde "$install_dir/$program"), linked as $(tilde "$bin_dir/$program")"
  if [ "$launcher_added" -eq 1 ]; then
    say "  start it:  $program, or \"Porthmos\" in your applications menu"
  else
    say "  start it:  $program"
  fi
  say "  remove it: curl -fsSL $base/uninstall.sh | sh"
  # shellcheck disable=SC2016
  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) printf 'Add %s to your PATH, for example:\n  export PATH="%s:$PATH"\n' "$bin_dir" "$bin_dir" ;;
  esac
}

if [ "${PORTHMOS_INSTALL_SOURCE_ONLY:-}" != 1 ]; then
  main "$@"
fi
