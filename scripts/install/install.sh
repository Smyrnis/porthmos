#!/bin/sh
set -eu

DEFAULT_BASE="https://github.com/Smyrnis/porthmos/releases/latest/download"

say() {
  printf '%s\n' "$*"
}

fail() {
  printf 'porthmos install: %s\n' "$*" >&2
  exit 1
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

main() {
  base=${PORTHMOS_RELEASE_BASE:-$DEFAULT_BASE}
  [ -n "${HOME:-}" ] || fail "HOME is not set"
  install_dir=${PORTHMOS_INSTALL_DIR:-$HOME/.local/share/porthmos}
  bin_dir=${PORTHMOS_BIN_DIR:-$HOME/.local/bin}

  case "$base" in
    https://*) set -- --proto '=https' --proto-redir '=https' --tlsv1.2 ;;
    file://*) set -- --proto '=file' ;;
    *) fail "the release location must start with https://" ;;
  esac
  command -v curl > /dev/null 2>&1 || fail "curl is required"

  if [ -n "${PORTHMOS_TARGET:-}" ]; then
    target=$PORTHMOS_TARGET
  else
    target=$(detect_target "$(uname -s)" "$(uname -m)") || exit 1
  fi

  temp=$(mktemp -d "${TMPDIR:-/tmp}/porthmos-install.XXXXXX") || fail "couldn't create a temporary folder"
  trap 'rm -rf "$temp"' EXIT
  trap 'exit 1' HUP INT TERM

  archive=porthmos-$target.tar.gz
  download "$@" -o "$temp/$archive" "$base/$archive" || fail "couldn't download $archive from $base"
  download "$@" -o "$temp/$archive.sha256" "$base/$archive.sha256" || fail "couldn't download $archive.sha256 from $base"
  verify_checksum "$temp" "$archive.sha256" || fail "the download does not match its checksum; nothing was installed"

  mkdir "$temp/extract"
  tar -xzf "$temp/$archive" -C "$temp/extract" porthmos || fail "the archive does not contain the porthmos program"

  mkdir -p "$install_dir" "$bin_dir" || fail "couldn't create $install_dir or $bin_dir"
  staged=$install_dir/.porthmos.new.$$
  if ! { cp "$temp/extract/porthmos" "$staged" && chmod 755 "$staged" && mv -f "$staged" "$install_dir/porthmos"; }; then
    rm -f "$staged"
    fail "couldn't install into $install_dir"
  fi
  ln -sf "$install_dir/porthmos" "$bin_dir/porthmos" || fail "couldn't link $bin_dir/porthmos"

  version=$("$install_dir/porthmos" --version 2> /dev/null || true)
  say "Installed ${version:-porthmos} in $install_dir and linked it as $bin_dir/porthmos"
  # shellcheck disable=SC2016
  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) printf 'Add %s to your PATH, for example:\n  export PATH="%s:$PATH"\n' "$bin_dir" "$bin_dir" ;;
  esac
}

if [ "${PORTHMOS_INSTALL_SOURCE_ONLY:-}" != 1 ]; then
  main "$@"
fi
