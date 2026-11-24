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
Usage: uninstall.sh [--purge] [--dry-run] [--yes]

Removes the Porthmos window app (porthmos-gui) that install.sh put in place, and
the old terminal app (porthmos) if an earlier install left it behind: the
programs in $PORTHMOS_INSTALL_DIR (default ~/.local/share/porthmos) and their
links in $PORTHMOS_BIN_DIR (default ~/.local/bin).

Your settings, saved connections, bookmarks, history and saved passwords are
kept unless you ask for them to go too.

  --purge   also remove the settings and state folders (including the old
            termconnect ones) and the saved passwords in the system keyring.
            ~/.ssh/known_hosts is never touched.
  --dry-run print what would be removed and change nothing: no question,
            nothing removed.
  --yes     do not ask before removing. Needed when the script is run without
            a terminal, for example through a pipe. The plan is still printed.
  --help    show this text.

Before it removes anything the script prints a plan and asks whether to
continue. The question is read from the terminal, so it works through
curl | sh as well.

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
  printf 'Continue? [y/N] ' >&2
  answer=$(head -n 1 2> /dev/null < "$tty") || fail "can't ask for confirmation without a terminal; add --yes to confirm"
  case $answer in
    [yY] | [yY][eE][sS]) return 0 ;;
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
tab=$(printf '\t')
nl='
'
actions=
found=0

tilde() {
  case $1 in
    "$HOME"/*) printf '~/%s\n' "${1#"$HOME"/}" ;;
    *) printf '%s\n' "$1" ;;
  esac
}

refuse_odd_name() {
  case $1 in
    *"$tab"* | *"$nl"*) fail "can't handle a path or folder name with a tab or a line break in it: $(printf '%s' "$1" | sed 's/^/ /' | head -n 1)" ;;
  esac
}

add_action() {
  refuse_odd_name "$2"
  actions="$actions$1$tab$2$tab$3
"
  found=1
}

collect_program() {
  name=$1
  link=$bin_dir/$name
  program=$install_dir/$name

  if [ -L "$link" ]; then
    target=$(readlink "$link")
    case $target in
      *"$tab"* | *"$nl"*) target="somewhere else" ;;
    esac
    if [ "$target" = "$program" ]; then
      add_action remove "$link" "the link"
    else
      add_action leave "$link" "it points to $target, not to the program the installer put in $install_dir"
    fi
  elif [ -e "$link" ]; then
    add_action leave "$link" "it is not the link the installer made"
  fi

  if [ -e "$program" ] || [ -L "$program" ]; then
    add_action remove "$program" "the program"
  fi

  if [ -e "$program.old" ] || [ -L "$program.old" ]; then
    add_action remove "$program.old" "a previous version"
  fi
}

data_home() {
  xdg_or "${XDG_DATA_HOME:-}" "$HOME/.local/share"
}

collect_launcher() {
  name=$1
  data=$(data_home)
  launcher=$data/applications/$name.desktop
  if [ -f "$launcher" ]; then
    case $(cat "$launcher" 2> /dev/null) in
      *X-Porthmos-Installer=install.sh*)
        add_action remove "$launcher" "the launcher"
        if [ -e "$data/icons/hicolor/scalable/apps/$name.svg" ]; then
          add_action remove "$data/icons/hicolor/scalable/apps/$name.svg" "the launcher icon"
        fi
        ;;
      *)
        found_before=$found
        add_action leave "$launcher" "the installer did not write it"
        found=$found_before
        ;;
    esac
  fi
}

collect_actions() {
  for name in $PROGRAMS; do
    collect_program "$name"
  done
  collect_launcher porthmos-gui

  for leftover in "$install_dir"/.porthmos.*; do
    [ -e "$leftover" ] || continue
    add_action remove "$leftover" "left by an interrupted install or update"
  done

  if [ -d "$install_dir" ]; then
    actions="$actions""remove$tab$install_dir${tab}the folder, once nothing else is in it
"
  fi
}

print_plan() {
  say "Porthmos uninstaller"
  say ""
  say "This will:"
  while IFS=$tab read -r verb path note; do
    [ -n "$verb" ] || continue
    printf '  %-7s %s   (%s)\n' "$verb" "$(tilde "$path")" "$note"
  done << PLAN
$actions
PLAN
  if [ "$purge" -eq 1 ]; then
    printf '%s' "$data_dirs" | while IFS= read -r directory; do
      [ -n "$directory" ] || continue
      printf '  %-7s %s   (%s)\n' remove "$(tilde "$directory")" "your settings and saved data, for good"
    done
    say "  remove  the saved passwords in the system keyring"
    say "  leave   ~/.ssh/known_hosts   (other programs use it)"
  else
    say "Your settings and saved connections are kept."
  fi
  say ""
}

apply_actions() {
  launcher_removed=0
  while IFS=$tab read -r verb path note; do
    [ -n "$verb" ] || continue
    case $verb in
      leave) say "Left $path alone: $note." ;;
      remove)
        [ "$path" = "$install_dir" ] && continue
        rm -f -- "$path"
        say "Removed $path"
        case $path in *.desktop) launcher_removed=1 ;; esac
        ;;
    esac
  done << APPLY
$actions
APPLY
  if [ "$launcher_removed" -eq 1 ] && command -v update-desktop-database > /dev/null 2>&1; then
    update-desktop-database "$(data_home)/applications" > /dev/null 2>&1 || true
  fi
  if [ -d "$install_dir" ]; then
    if ! rmdir "$install_dir" 2> /dev/null; then
      say "Left $install_dir in place: it still holds files that are not Porthmos's."
    fi
  fi
}

main() {
  purge=0
  yes=0
  dry_run=0
  for argument in "$@"; do
    case $argument in
      --purge) purge=1 ;;
      --yes | -y) yes=1 ;;
      --dry-run) dry_run=1 ;;
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
    refuse_odd_name "$candidate"
    data_dirs="$data_dirs$candidate
"
  done

  collect_actions

  if [ "$found" -eq 0 ]; then
    if [ "$purge" -eq 0 ] || [ -z "$data_dirs" ]; then
      fail "nothing to remove: no Porthmos program in $install_dir. If you installed Porthmos with a package manager, Homebrew or cargo install, remove it with that tool."
    fi
  fi

  print_plan
  if [ "$dry_run" -eq 1 ]; then
    say "Dry run: nothing was removed."
    exit 0
  fi
  if [ "$yes" -eq 0 ] && ! confirm; then
    say "Stopped; nothing was removed."
    exit 1
  fi

  apply_actions

  if [ "$found" -eq 0 ]; then
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
