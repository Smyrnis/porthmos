#!/bin/sh
set -eu

RELEASES="https://github.com/Smyrnis/porthmos/releases/download"

say() {
  printf '%s\n' "$*"
}

fail() {
  printf 'porthmos install: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat << 'USAGE'
Usage: install.sh [--gui] [--preview]

Installs Porthmos into $PORTHMOS_INSTALL_DIR (default ~/.local/share/porthmos)
and links it from $PORTHMOS_BIN_DIR (default ~/.local/bin).

  --gui       install the window app (porthmos-gui) instead of the terminal
              app (porthmos). The two are separate programs: installing one
              never touches the other.
  --preview   install the preview build instead of the stable one. Preview
              builds come from the development branch, get new features
              first, and may be less polished. They update from the preview
              feed. Run this script again without --preview to go back to
              the stable build.
  --help      show this text.
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
  curl -fsSL "$@" || return 1
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

fetch_verified() {
  temp=$1
  base=$2
  archive=$3
  shift 3
  attempt=1
  while :; do
    download "$@" -o "$temp/$archive" "$base/$archive" || fail "couldn't download $archive from $base"
    download "$@" -o "$temp/$archive.sha256" "$base/$archive.sha256" || fail "couldn't download $archive.sha256 from $base"
    verify_checksum "$temp" "$archive.sha256" && return 0
    [ "$attempt" -lt 2 ] || fail "the download does not match its checksum; nothing was installed"
    say "The download does not match its checksum; the release may be mid-upload. Trying once more."
    attempt=$((attempt + 1))
    sleep "${PORTHMOS_RETRY_DELAY:-3}"
  done
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
  [ "$(uname -s)" = Linux ] || return 0
  [ -f "$icon" ] || return 0
  case $executable in
    *'"'* | *'$'* | *'`'* | *\\* | *'%'*)
      say "Couldn't add the launcher: the install path has a character a launcher can't hold."
      return 0
      ;;
  esac
  data=$(data_home)
  icon_dir=$data/icons/hicolor/scalable/apps
  app_dir=$data/applications
  pending=$app_dir/.$program.desktop.$$
  if ! { mkdir -p "$icon_dir" "$app_dir" && cp "$icon" "$icon_dir/$program.svg" &&
    write_launcher "$executable" "$program" > "$pending" && mv -f "$pending" "$app_dir/$program.desktop"; }; then
    rm -f "$pending"
    say "Couldn't add the launcher to the applications menu; $program still runs from the command line."
    return 0
  fi
  if command -v update-desktop-database > /dev/null 2>&1; then
    update-desktop-database "$app_dir" > /dev/null 2>&1 || true
  fi
}

main() {
  channel=stable
  product=tui
  program=porthmos
  for argument in "$@"; do
    case $argument in
      --preview) channel=preview ;;
      --gui)
        product=gui
        program=porthmos-gui
        ;;
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
  command -v curl > /dev/null 2>&1 || fail "curl is required"
  if [ "$channel" = preview ]; then
    say "Installing the preview build of $program from the development channel. It may be less stable than the release."
  fi

  if [ -n "${PORTHMOS_TARGET:-}" ]; then
    target=$PORTHMOS_TARGET
  else
    target=$(detect_target "$(uname -s)" "$(uname -m)") || exit 1
  fi

  temp=$(mktemp -d "${TMPDIR:-/tmp}/porthmos-install.XXXXXX") || fail "couldn't create a temporary folder"
  trap 'rm -rf "$temp"' EXIT
  trap 'exit 1' HUP INT TERM

  archive=$program-$target.tar.gz
  fetch_verified "$temp" "$base" "$archive" "$@"

  mkdir "$temp/extract"
  tar -xzf "$temp/$archive" -C "$temp/extract" "$program" || fail "the archive does not contain the $program program"
  if [ "$product" = gui ]; then
    tar -xzf "$temp/$archive" -C "$temp/extract" "$program.svg" > /dev/null 2>&1 || true
  fi

  mkdir -p "$install_dir" "$bin_dir" || fail "couldn't create $install_dir or $bin_dir"
  staged=$install_dir/.porthmos.new.$$
  if ! { cp "$temp/extract/$program" "$staged" && chmod 755 "$staged" && mv -f "$staged" "$install_dir/$program"; }; then
    rm -f "$staged"
    fail "couldn't install into $install_dir"
  fi
  ln -sf "$install_dir/$program" "$bin_dir/$program" || fail "couldn't link $bin_dir/$program"
  if [ "$product" = gui ]; then
    install_launcher "$program" "$install_dir/$program" "$temp/extract/$program.svg"
  fi

  version=$("$install_dir/$program" --version 2> /dev/null || true)
  say "Installed ${version:-$program} in $install_dir and linked it as $bin_dir/$program"
  # shellcheck disable=SC2016
  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) printf 'Add %s to your PATH, for example:\n  export PATH="%s:$PATH"\n' "$bin_dir" "$bin_dir" ;;
  esac
}

if [ "${PORTHMOS_INSTALL_SOURCE_ONLY:-}" != 1 ]; then
  main "$@"
fi
