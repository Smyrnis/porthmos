#!/bin/sh
set -eu

say() {
  printf '%s\n' "$*"
}

fail() {
  printf 'porthmos uninstall: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat << 'USAGE'
Usage: uninstall.sh [--purge] [--yes]

Removes the Porthmos programs that install.sh put in place, the terminal app
(porthmos) and the window app (porthmos-gui), whichever are installed: the
programs in $PORTHMOS_INSTALL_DIR (default ~/.local/share/porthmos) and their
links in $PORTHMOS_BIN_DIR (default ~/.local/bin).

Your settings, saved connections, bookmarks, history and saved passwords are
kept unless you ask for them to go too.

  --purge   also remove the settings and state folders (including the old
            termconnect ones) and the saved passwords in the system keyring.
            ~/.ssh/known_hosts is never touched.
  --yes     do not ask before --purge removes anything. Needed when the script
            is run without a terminal, for example through a pipe.
  --help    show this text.

Programs installed another way (a package manager, Homebrew, cargo install)
are removed with that tool, not with this script.
USAGE
}

xdg_or() {
  case $1 in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s\n' "$2" ;;
  esac
}

confirm() {
  tty=${PORTHMOS_UNINSTALL_TTY:-/dev/tty}
  answer=$(head -n 1 2> /dev/null < "$tty") || fail "can't ask for confirmation without a terminal; add --yes to confirm"
  case $answer in
    y | Y | yes | YES | Yes) return 0 ;;
    *) return 1 ;;
  esac
}

clear_keyring() {
  case $(uname -s) in
    Darwin)
      if command -v security > /dev/null 2>&1; then
        count=0
        while [ "$count" -lt 100 ] && security delete-generic-password -s porthmos > /dev/null 2>&1; do
          count=$((count + 1))
        done
        say "Removed Porthmos's saved passwords from the keychain."
      else
        say "Saved passwords were not touched: the security tool was not found. Look for \"porthmos\" in Keychain Access."
      fi
      ;;
    *)
      if command -v secret-tool > /dev/null 2>&1; then
        secret-tool clear service porthmos > /dev/null 2>&1 || true
        say "Removed Porthmos's saved passwords from the system keyring."
      else
        say "Saved passwords were not touched: secret-tool was not found. Look for \"porthmos\" in your keyring application."
      fi
      ;;
  esac
}

PROGRAMS="porthmos porthmos-gui"

remove_one_program() {
  name=$1
  link=$bin_dir/$name
  program=$install_dir/$name

  if [ -L "$link" ]; then
    target=$(readlink "$link")
    if [ "$target" = "$program" ]; then
      rm -f "$link"
      say "Removed $link"
      found=1
    else
      say "Left $link alone: it points to $target, not to the program the installer put in $install_dir."
      found=1
    fi
  elif [ -e "$link" ]; then
    say "Left $link alone: it is not the link the installer made."
    found=1
  fi

  if [ -e "$program" ] || [ -L "$program" ]; then
    rm -f "$program"
    say "Removed $program"
    found=1
  fi

  if [ -e "$program.old" ] || [ -L "$program.old" ]; then
    rm -f "$program.old"
    found=1
  fi
}

data_home() {
  xdg_or "${XDG_DATA_HOME:-}" "$HOME/.local/share"
}

remove_launcher() {
  name=$1
  data=$(data_home)
  launcher=$data/applications/$name.desktop
  if [ -f "$launcher" ]; then
    case $(cat "$launcher" 2> /dev/null) in
      *X-Porthmos-Installer=install.sh*)
        rm -f "$launcher"
        say "Removed $launcher"
        found=1
        rm -f "$data/icons/hicolor/scalable/apps/$name.svg"
        if command -v update-desktop-database > /dev/null 2>&1; then
          update-desktop-database "$data/applications" > /dev/null 2>&1 || true
        fi
        ;;
      *) say "Left $launcher alone: the installer did not write it." ;;
    esac
  fi
}

remove_program() {
  found=0
  for name in $PROGRAMS; do
    remove_one_program "$name"
  done
  remove_launcher porthmos-gui

  for leftover in "$install_dir"/.porthmos.*; do
    [ -e "$leftover" ] || continue
    rm -f "$leftover"
    found=1
  done

  if [ -d "$install_dir" ]; then
    if ! rmdir "$install_dir" 2> /dev/null; then
      say "Left $install_dir in place: it still holds files that are not Porthmos's."
    fi
  fi
}

main() {
  purge=0
  yes=0
  for argument in "$@"; do
    case $argument in
      --purge) purge=1 ;;
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

  [ -n "${HOME:-}" ] || fail "HOME is not set"
  install_dir=${PORTHMOS_INSTALL_DIR:-$HOME/.local/share/porthmos}
  bin_dir=${PORTHMOS_BIN_DIR:-$HOME/.local/bin}
  config_base=$(xdg_or "${XDG_CONFIG_HOME:-}" "$HOME/.config")
  state_base=$(xdg_or "${XDG_STATE_HOME:-}" "$HOME/.local/state")

  data_dirs=
  for candidate in "$config_base/porthmos" "$config_base/termconnect" "$state_base/porthmos" "$state_base/termconnect"; do
    [ -d "$candidate" ] || continue
    data_dirs="$data_dirs$candidate
"
  done

  if [ "$purge" -eq 1 ] && [ "$yes" -eq 0 ]; then
    say "This will also remove, for good:"
    printf '%s' "$data_dirs" | sed 's/^/  /'
    say "  and the saved passwords in the system keyring"
    printf 'Remove them? [y/N] '
    confirm || fail "stopped; nothing was removed"
  fi

  remove_program

  if [ "$found" -eq 0 ]; then
    if [ "$purge" -eq 0 ] || [ -z "$data_dirs" ]; then
      fail "nothing to remove: no Porthmos program in $install_dir. If you installed Porthmos with a package manager, Homebrew or cargo install, remove it with that tool."
    fi
    say "No program found in $install_dir; removing only the saved data."
  fi

  if [ "$purge" -eq 1 ]; then
    printf '%s' "$data_dirs" | while IFS= read -r directory; do
      [ -n "$directory" ] || continue
      rm -rf -- "$directory"
      say "Removed $directory"
    done
    clear_keyring
    say "Left ~/.ssh/known_hosts alone: other programs use it."
  elif [ -n "$data_dirs" ]; then
    say "Kept your settings and saved data:"
    printf '%s' "$data_dirs" | sed 's/^/  /'
    say "Saved passwords stay in the system keyring. Run this script again with --purge to remove all of it."
  fi
}

if [ "${PORTHMOS_UNINSTALL_SOURCE_ONLY:-}" != 1 ]; then
  main "$@"
fi
