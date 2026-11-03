#!/usr/bin/env bash
# shellcheck disable=SC2030,SC2031
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
INSTALL_SH="$ROOT/scripts/install/install.sh"
UNINSTALL_SH="$ROOT/scripts/install/uninstall.sh"
WORK=$(mktemp -d)
FAKE_BIN="$WORK/fakebin"
KEYRING_LOG="$WORK/keyring.log"
trap 'rm -rf "$WORK"' EXIT
TARGET=x86_64-unknown-linux-gnu
failures=0
unset XDG_CONFIG_HOME XDG_STATE_HOME PORTHMOS_INSTALL_DIR PORTHMOS_BIN_DIR

release_tool() {
  (cd "$ROOT" && cargo run -q -p porthmos-release -- "$@")
}

make_release() {
  local dir=$1 marker=$2
  mkdir -p "$dir"
  printf '#!/bin/sh\necho "porthmos-stub %s"\n' "$marker" > "$dir/porthmos"
  chmod 755 "$dir/porthmos"
  printf 'MIT\n' > "$dir/LICENSE"
  release_tool package --target "$TARGET" --version 0.4.0 --binary "$dir/porthmos" --license "$dir/LICENSE" --out "$dir/dist" > /dev/null
}

run_install() {
  local home=$1 base=$2
  shift 2
  HOME="$home" PORTHMOS_RELEASE_BASE="$base" PORTHMOS_TARGET="$TARGET" TMPDIR="$WORK/tmp" "$@" sh "$INSTALL_SH"
}

run_install_channel() {
  local home=$1 stable=$2 preview=$3
  shift 3
  HOME="$home" PORTHMOS_RELEASE_BASE="$stable" PORTHMOS_PREVIEW_BASE="$preview" PORTHMOS_TARGET="$TARGET" TMPDIR="$WORK/tmp" sh "$INSTALL_SH" "$@"
}

fresh_home() {
  mkdir -p "$WORK/tmp"
  printf '%s\n' "$WORK/home-$1"
}

test_maps_each_supported_os_and_machine_to_its_target() {
  (
    export PORTHMOS_INSTALL_SOURCE_ONLY=1
    # shellcheck disable=SC1090
    . "$INSTALL_SH"
    [ "$(detect_target Linux x86_64)" = x86_64-unknown-linux-gnu ]
    [ "$(detect_target Linux aarch64)" = aarch64-unknown-linux-gnu ]
    [ "$(detect_target Linux arm64)" = aarch64-unknown-linux-gnu ]
    [ "$(detect_target Darwin x86_64)" = x86_64-apple-darwin ]
    [ "$(detect_target Darwin arm64)" = aarch64-apple-darwin ]
  )
}

test_rejects_an_unsupported_platform() {
  (
    export PORTHMOS_INSTALL_SOURCE_ONLY=1
    # shellcheck disable=SC1090
    . "$INSTALL_SH"
    if detect_target FreeBSD amd64 > "$WORK/out-unsupported" 2>&1; then exit 1; fi
    grep -q 'unsupported platform: FreeBSD amd64' "$WORK/out-unsupported"
  )
}

test_fresh_install_places_the_binary_links_it_and_it_runs() {
  local home
  home=$(fresh_home fresh)
  make_release "$WORK/rel-fresh" one
  run_install "$home" "file://$WORK/rel-fresh/dist" > "$WORK/out-fresh" 2>&1
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub one" ]
  [ -x "$home/.local/share/porthmos/porthmos" ]
  [ "$(readlink "$home/.local/bin/porthmos")" = "$home/.local/share/porthmos/porthmos" ]
  grep -q "$home/.local/share/porthmos" "$WORK/out-fresh"
}

test_running_it_again_upgrades_in_place() {
  local home
  home=$(fresh_home upgrade)
  make_release "$WORK/rel-up-1" one
  make_release "$WORK/rel-up-2" two
  run_install "$home" "file://$WORK/rel-up-1/dist" > /dev/null 2>&1
  run_install "$home" "file://$WORK/rel-up-2/dist" > /dev/null 2>&1
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub two" ]
}

corrupt() {
  make_release "$1-tampered" tampered
  cp "$1-tampered/dist/porthmos-$TARGET.tar.gz" "$1/dist/porthmos-$TARGET.tar.gz"
}

test_a_corrupted_archive_installs_nothing_and_exits_non_zero() {
  local home
  home=$(fresh_home corrupt)
  make_release "$WORK/rel-corrupt" one
  corrupt "$WORK/rel-corrupt"
  if run_install "$home" "file://$WORK/rel-corrupt/dist" > /dev/null 2>&1; then return 1; fi
  [ ! -e "$home/.local/share/porthmos/porthmos" ]
  [ ! -e "$home/.local/bin/porthmos" ]
}

test_a_corrupted_archive_leaves_an_existing_install_untouched() {
  local home
  home=$(fresh_home corrupt-existing)
  make_release "$WORK/rel-ce-1" one
  make_release "$WORK/rel-ce-2" two
  run_install "$home" "file://$WORK/rel-ce-1/dist" > /dev/null 2>&1
  corrupt "$WORK/rel-ce-2"
  if run_install "$home" "file://$WORK/rel-ce-2/dist" > /dev/null 2>&1; then return 1; fi
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub one" ]
}

test_a_missing_checksum_file_installs_nothing() {
  local home
  home=$(fresh_home nosum)
  make_release "$WORK/rel-nosum" one
  rm "$WORK/rel-nosum/dist/porthmos-$TARGET.tar.gz.sha256"
  if run_install "$home" "file://$WORK/rel-nosum/dist" > /dev/null 2>&1; then return 1; fi
  [ ! -e "$home/.local/bin/porthmos" ]
}

test_the_stable_channel_is_the_default_and_the_preview_channel_has_its_own_location() {
  (
    export PORTHMOS_INSTALL_SOURCE_ONLY=1
    # shellcheck disable=SC1090
    . "$INSTALL_SH"
    [ "$(release_base stable)" = "https://github.com/Smyrnis/porthmos/releases/latest/download" ]
    [ "$(release_base preview)" = "https://github.com/Smyrnis/porthmos/releases/download/preview" ]
  )
}

test_each_channel_location_can_be_overridden_on_its_own() {
  (
    export PORTHMOS_INSTALL_SOURCE_ONLY=1
    # shellcheck disable=SC1090
    . "$INSTALL_SH"
    export PORTHMOS_RELEASE_BASE=https://stable.example/x
    export PORTHMOS_PREVIEW_BASE=https://preview.example/y
    [ "$(release_base stable)" = https://stable.example/x ]
    [ "$(release_base preview)" = https://preview.example/y ]
  )
}

test_without_the_flag_the_stable_build_is_installed() {
  local home
  home=$(fresh_home chan-stable)
  make_release "$WORK/rel-chan-stable" stable
  make_release "$WORK/rel-chan-preview" preview
  run_install_channel "$home" "file://$WORK/rel-chan-stable/dist" "file://$WORK/rel-chan-preview/dist" > "$WORK/out-chan-stable" 2>&1
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub stable" ]
  ! grep -qi 'preview' "$WORK/out-chan-stable"
}

test_the_preview_flag_installs_the_preview_build_and_says_so() {
  local home
  home=$(fresh_home chan-preview)
  make_release "$WORK/rel-chan-stable" stable
  make_release "$WORK/rel-chan-preview" preview
  run_install_channel "$home" "file://$WORK/rel-chan-stable/dist" "file://$WORK/rel-chan-preview/dist" --preview > "$WORK/out-chan-preview" 2>&1
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub preview" ]
  grep -qi 'preview' "$WORK/out-chan-preview"
  grep -q 'development' "$WORK/out-chan-preview"
}

test_the_stable_flag_less_run_replaces_a_preview_install() {
  local home
  home=$(fresh_home chan-back)
  make_release "$WORK/rel-chan-stable" stable
  make_release "$WORK/rel-chan-preview" preview
  run_install_channel "$home" "file://$WORK/rel-chan-stable/dist" "file://$WORK/rel-chan-preview/dist" --preview > /dev/null 2>&1
  run_install_channel "$home" "file://$WORK/rel-chan-stable/dist" "file://$WORK/rel-chan-preview/dist" > /dev/null 2>&1
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub stable" ]
}

test_a_preview_install_over_a_stable_one_replaces_it() {
  local home
  home=$(fresh_home chan-forward)
  make_release "$WORK/rel-chan-stable" stable
  make_release "$WORK/rel-chan-preview" preview
  run_install_channel "$home" "file://$WORK/rel-chan-stable/dist" "file://$WORK/rel-chan-preview/dist" > /dev/null 2>&1
  run_install_channel "$home" "file://$WORK/rel-chan-stable/dist" "file://$WORK/rel-chan-preview/dist" --preview > /dev/null 2>&1
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub preview" ]
}

test_the_preview_location_must_be_https_or_a_file() {
  local home
  home=$(fresh_home chan-http)
  mkdir -p "$WORK/tmp"
  if run_install_channel "$home" "file://$WORK/none" "http://example.invalid/preview" --preview > "$WORK/out-chan-http" 2>&1; then return 1; fi
  grep -q 'https' "$WORK/out-chan-http"
  [ ! -e "$home/.local/bin/porthmos" ]
}

test_an_unknown_install_option_is_refused_and_installs_nothing() {
  local home
  home=$(fresh_home chan-badopt)
  make_release "$WORK/rel-chan-stable" stable
  if run_install_channel "$home" "file://$WORK/rel-chan-stable/dist" "file://$WORK/none" --nightly > "$WORK/out-chan-badopt" 2>&1; then return 1; fi
  grep -qi 'usage' "$WORK/out-chan-badopt"
  [ ! -e "$home/.local/bin/porthmos" ]
}

test_install_help_describes_the_preview_flag_and_installs_nothing() {
  local home
  home=$(fresh_home chan-help)
  mkdir -p "$home"
  run_install_channel "$home" "file://$WORK/none" "file://$WORK/none" --help > "$WORK/out-chan-help" 2>&1
  grep -q -- '--preview' "$WORK/out-chan-help"
  [ ! -e "$home/.local/bin/porthmos" ]
}

test_works_when_home_contains_a_space_and_the_bin_dir_does_not_exist() {
  local home="$WORK/home with space"
  mkdir -p "$WORK/tmp"
  make_release "$WORK/rel-space" one
  run_install "$home" "file://$WORK/rel-space/dist" > /dev/null 2>&1
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub one" ]
}

test_warns_with_an_export_line_when_the_bin_dir_is_not_on_path() {
  local home
  home=$(fresh_home nopath)
  make_release "$WORK/rel-nopath" one
  run_install "$home" "file://$WORK/rel-nopath/dist" env PATH=/usr/bin:/bin > "$WORK/out-nopath" 2>&1
  grep -qF "export PATH=\"$home/.local/bin:\$PATH\"" "$WORK/out-nopath"
}

test_stays_silent_about_path_when_the_bin_dir_is_on_it() {
  local home
  home=$(fresh_home onpath)
  make_release "$WORK/rel-onpath" one
  run_install "$home" "file://$WORK/rel-onpath/dist" env "PATH=$home/.local/bin:/usr/bin:/bin" > "$WORK/out-onpath" 2>&1
  ! grep -q 'export PATH' "$WORK/out-onpath"
}

test_refuses_a_plain_http_base() {
  local home
  home=$(fresh_home http)
  mkdir -p "$WORK/tmp"
  if run_install "$home" "http://example.invalid/releases" > "$WORK/out-http" 2>&1; then return 1; fi
  grep -q 'https' "$WORK/out-http"
  [ ! -e "$home/.local/bin/porthmos" ]
}

test_leaves_no_temp_files_behind() {
  local home
  home=$(fresh_home temps)
  make_release "$WORK/rel-temps" one
  rm -rf "$WORK/tmp"
  mkdir -p "$WORK/tmp"
  run_install "$home" "file://$WORK/rel-temps/dist" > /dev/null 2>&1
  [ -z "$(ls -A "$WORK/tmp")" ]
  corrupt "$WORK/rel-temps"
  run_install "$home" "file://$WORK/rel-temps/dist" > /dev/null 2>&1 || true
  [ -z "$(ls -A "$WORK/tmp")" ]
}


make_fake_keyring() {
  mkdir -p "$FAKE_BIN"
  for tool in secret-tool security; do
    {
      printf '#!/bin/sh\n'
      # shellcheck disable=SC2016
      printf 'printf "%%s %%s\\n" "%s" "$*" >> "%s"\n' "$tool" "$KEYRING_LOG"
      printf 'exit 1\n'
    } > "$FAKE_BIN/$tool"
    chmod 755 "$FAKE_BIN/$tool"
  done
  : > "$KEYRING_LOG"
}

guard_keyring() {
  for tool in secret-tool security; do
    [ "$(PATH="$FAKE_BIN:$PATH" command -v "$tool")" = "$FAKE_BIN/$tool" ] || exit 99
  done
}

run_uninstall() {
  local home=$1
  shift
  guard_keyring
  HOME="$home" PATH="$FAKE_BIN:$PATH" PORTHMOS_UNINSTALL_TTY="$WORK/no-terminal" TMPDIR="$WORK/tmp" sh "$UNINSTALL_SH" "$@"
}

run_uninstall_asking() {
  local home=$1 answer_file=$2
  shift 2
  guard_keyring
  HOME="$home" PATH="$FAKE_BIN:$PATH" PORTHMOS_UNINSTALL_TTY="$answer_file" TMPDIR="$WORK/tmp" sh "$UNINSTALL_SH" "$@"
}

bare_path() {
  local bin="$WORK/barebin"
  mkdir -p "$bin"
  for tool in head sed rm rmdir readlink uname cat; do
    ln -sf "$(command -v "$tool")" "$bin/$tool"
  done
  printf '%s\n' "$bin"
}

fake_install() {
  local home=$1
  mkdir -p "$home/.local/share/porthmos" "$home/.local/bin"
  printf '#!/bin/sh\necho stub\n' > "$home/.local/share/porthmos/porthmos"
  chmod 755 "$home/.local/share/porthmos/porthmos"
  ln -s "$home/.local/share/porthmos/porthmos" "$home/.local/bin/porthmos"
}

fake_data() {
  local home=$1
  mkdir -p "$home/.config/porthmos" "$home/.local/state/porthmos/updates" "$home/.config/other" "$home/.ssh"
  printf 'x\n' > "$home/.config/porthmos/connections.toml"
  printf 'x\n' > "$home/.local/state/porthmos/state.toml"
  printf 'keep\n' > "$home/.config/other/keep.toml"
  printf 'keep\n' > "$home/.ssh/known_hosts"
}

test_the_tests_do_not_inherit_the_callers_config_folders() {
  [ -z "${XDG_CONFIG_HOME:-}" ]
  [ -z "${XDG_STATE_HOME:-}" ]
  [ -z "${PORTHMOS_INSTALL_DIR:-}" ]
  [ -z "${PORTHMOS_BIN_DIR:-}" ]
}

test_the_uninstall_tests_never_reach_a_real_keyring_tool() {
  guard_keyring
  [ "$(PATH="$FAKE_BIN:$PATH" command -v secret-tool)" = "$FAKE_BIN/secret-tool" ]
  [ "$(PATH="$FAKE_BIN:$PATH" command -v security)" = "$FAKE_BIN/security" ]
}

test_uninstall_removes_the_program_and_its_link_and_the_empty_folder() {
  local home
  home=$(fresh_home un-basic)
  fake_install "$home"
  run_uninstall "$home" > "$WORK/out-un-basic" 2>&1
  [ ! -e "$home/.local/share/porthmos/porthmos" ]
  [ ! -L "$home/.local/bin/porthmos" ]
  [ ! -e "$home/.local/share/porthmos" ]
  [ -d "$home/.local/bin" ]
}

test_uninstall_removes_files_left_by_an_interrupted_update() {
  local home
  home=$(fresh_home un-leftovers)
  fake_install "$home"
  : > "$home/.local/share/porthmos/.porthmos.update.old.123"
  : > "$home/.local/share/porthmos/.porthmos.new.456"
  run_uninstall "$home" > /dev/null 2>&1
  [ ! -e "$home/.local/share/porthmos" ]
}

test_uninstall_keeps_files_it_does_not_own_in_the_install_folder() {
  local home
  home=$(fresh_home un-foreign)
  fake_install "$home"
  printf 'mine\n' > "$home/.local/share/porthmos/notes.txt"
  run_uninstall "$home" > "$WORK/out-un-foreign" 2>&1
  [ ! -e "$home/.local/share/porthmos/porthmos" ]
  [ -f "$home/.local/share/porthmos/notes.txt" ]
  grep -q "$home/.local/share/porthmos" "$WORK/out-un-foreign"
}

test_uninstall_keeps_settings_and_data_by_default_and_says_where_they_are() {
  local home
  home=$(fresh_home un-keep)
  fake_install "$home"
  fake_data "$home"
  : > "$KEYRING_LOG"
  run_uninstall "$home" > "$WORK/out-un-keep" 2>&1
  [ -f "$home/.config/porthmos/connections.toml" ]
  [ -f "$home/.local/state/porthmos/state.toml" ]
  [ ! -s "$KEYRING_LOG" ]
  grep -q "$home/.config/porthmos" "$WORK/out-un-keep"
  grep -q "$home/.local/state/porthmos" "$WORK/out-un-keep"
  grep -q -- '--purge' "$WORK/out-un-keep"
}

test_uninstall_leaves_a_link_that_points_somewhere_else() {
  local home
  home=$(fresh_home un-otherlink)
  fake_install "$home"
  rm "$home/.local/bin/porthmos"
  ln -s /bin/sh "$home/.local/bin/porthmos"
  run_uninstall "$home" > "$WORK/out-un-otherlink" 2>&1
  [ -L "$home/.local/bin/porthmos" ]
  [ "$(readlink "$home/.local/bin/porthmos")" = /bin/sh ]
  [ ! -e "$home/.local/share/porthmos/porthmos" ]
  grep -q "$home/.local/bin/porthmos" "$WORK/out-un-otherlink"
}

test_uninstall_leaves_a_regular_file_in_the_bin_folder() {
  local home
  home=$(fresh_home un-regular)
  fake_install "$home"
  rm "$home/.local/bin/porthmos"
  printf 'mine\n' > "$home/.local/bin/porthmos"
  run_uninstall "$home" > /dev/null 2>&1
  [ "$(cat "$home/.local/bin/porthmos")" = mine ]
}

test_uninstall_reports_when_nothing_is_installed_and_points_at_other_methods() {
  local home
  home=$(fresh_home un-nothing)
  mkdir -p "$home"
  if run_uninstall "$home" > "$WORK/out-un-nothing" 2>&1; then return 1; fi
  grep -qi 'package manager' "$WORK/out-un-nothing"
  grep -q "$home/.local/share/porthmos" "$WORK/out-un-nothing"
}

test_uninstall_follows_the_same_folder_overrides_as_the_installer() {
  local home
  home=$(fresh_home un-override)
  mkdir -p "$home/apps/porthmos" "$home/links"
  printf '#!/bin/sh\n' > "$home/apps/porthmos/porthmos"
  ln -s "$home/apps/porthmos/porthmos" "$home/links/porthmos"
  PORTHMOS_INSTALL_DIR="$home/apps/porthmos" PORTHMOS_BIN_DIR="$home/links" run_uninstall "$home" > /dev/null 2>&1
  [ ! -e "$home/apps/porthmos/porthmos" ]
  [ ! -L "$home/links/porthmos" ]
}

test_uninstall_works_when_home_contains_a_space() {
  local home="$WORK/home with space un"
  mkdir -p "$WORK/tmp"
  fake_install "$home"
  run_uninstall "$home" > /dev/null 2>&1
  [ ! -e "$home/.local/share/porthmos" ]
  [ ! -L "$home/.local/bin/porthmos" ]
}

test_purge_with_yes_removes_settings_state_and_the_old_name_folders_only() {
  local home
  home=$(fresh_home un-purge)
  fake_install "$home"
  fake_data "$home"
  mkdir -p "$home/.config/termconnect" "$home/.local/state/termconnect"
  run_uninstall "$home" --purge --yes > "$WORK/out-un-purge" 2>&1
  [ ! -e "$home/.config/porthmos" ]
  [ ! -e "$home/.local/state/porthmos" ]
  [ ! -e "$home/.config/termconnect" ]
  [ ! -e "$home/.local/state/termconnect" ]
  [ -f "$home/.config/other/keep.toml" ]
  [ -f "$home/.ssh/known_hosts" ]
  [ ! -e "$home/.local/share/porthmos" ]
}

test_purge_follows_xdg_folders_when_they_are_absolute() {
  local home
  home=$(fresh_home un-xdg)
  fake_install "$home"
  mkdir -p "$home/xdg-config/porthmos" "$home/xdg-state/porthmos" "$home/.config/porthmos"
  XDG_CONFIG_HOME="$home/xdg-config" XDG_STATE_HOME="$home/xdg-state" run_uninstall "$home" --purge --yes > /dev/null 2>&1
  [ ! -e "$home/xdg-config/porthmos" ]
  [ ! -e "$home/xdg-state/porthmos" ]
  [ -d "$home/.config/porthmos" ]
  [ -d "$home/xdg-config" ]
}

test_purge_ignores_a_relative_xdg_folder_like_the_app_does() {
  local home
  home=$(fresh_home un-xdg-relative)
  fake_install "$home"
  fake_data "$home"
  mkdir -p "$WORK/relative/porthmos"
  (cd "$WORK" && XDG_CONFIG_HOME=relative run_uninstall "$home" --purge --yes > /dev/null 2>&1)
  [ -d "$WORK/relative/porthmos" ]
  [ ! -e "$home/.config/porthmos" ]
}

test_purge_without_yes_and_without_a_terminal_removes_nothing() {
  local home
  home=$(fresh_home un-noterm)
  fake_install "$home"
  fake_data "$home"
  if run_uninstall "$home" --purge > "$WORK/out-un-noterm" 2>&1; then return 1; fi
  [ -f "$home/.config/porthmos/connections.toml" ]
  [ -x "$home/.local/share/porthmos/porthmos" ]
  grep -q -- '--yes' "$WORK/out-un-noterm"
}

test_purge_asks_on_the_terminal_and_a_no_removes_nothing() {
  local home
  home=$(fresh_home un-no)
  fake_install "$home"
  fake_data "$home"
  printf 'n\n' > "$WORK/answer-no"
  if run_uninstall_asking "$home" "$WORK/answer-no" --purge > /dev/null 2>&1; then return 1; fi
  [ -f "$home/.config/porthmos/connections.toml" ]
  [ -x "$home/.local/share/porthmos/porthmos" ]
}

test_purge_asks_on_the_terminal_and_a_yes_removes_everything() {
  local home
  home=$(fresh_home un-yes)
  fake_install "$home"
  fake_data "$home"
  printf 'y\n' > "$WORK/answer-yes"
  run_uninstall_asking "$home" "$WORK/answer-yes" --purge > /dev/null 2>&1
  [ ! -e "$home/.config/porthmos" ]
  [ ! -e "$home/.local/share/porthmos" ]
}

test_purge_clears_saved_passwords_from_the_system_keyring() {
  local home
  home=$(fresh_home un-keyring)
  fake_install "$home"
  fake_data "$home"
  : > "$KEYRING_LOG"
  run_uninstall "$home" --purge --yes > /dev/null 2>&1
  case "$(uname -s)" in
    Darwin) grep -qx 'security delete-generic-password -s porthmos' "$KEYRING_LOG" ;;
    *) grep -qx 'secret-tool clear service porthmos' "$KEYRING_LOG" ;;
  esac
}

test_purge_still_finishes_when_there_is_no_keyring_tool() {
  local home bare
  home=$(fresh_home un-nokeyring)
  bare=$(bare_path)
  fake_install "$home"
  fake_data "$home"
  : > "$KEYRING_LOG"
  HOME="$home" PATH="$bare" /bin/sh "$UNINSTALL_SH" --purge --yes > "$WORK/out-un-nokeyring" 2>&1
  [ ! -e "$home/.config/porthmos" ]
  [ ! -s "$KEYRING_LOG" ]
  grep -qi 'not touched' "$WORK/out-un-nokeyring"
}

test_purge_names_what_it_removed_and_what_it_left() {
  local home
  home=$(fresh_home un-report)
  fake_install "$home"
  fake_data "$home"
  run_uninstall "$home" --purge --yes > "$WORK/out-un-report" 2>&1
  grep -q "$home/.config/porthmos" "$WORK/out-un-report"
  grep -q 'known_hosts' "$WORK/out-un-report"
}

test_purge_after_a_plain_uninstall_still_removes_the_data() {
  local home
  home=$(fresh_home un-then-purge)
  fake_install "$home"
  fake_data "$home"
  run_uninstall "$home" > /dev/null 2>&1
  [ -f "$home/.config/porthmos/connections.toml" ]
  run_uninstall "$home" --purge --yes > "$WORK/out-un-then-purge" 2>&1
  [ ! -e "$home/.config/porthmos" ]
  [ ! -e "$home/.local/state/porthmos" ]
}

test_purge_with_no_program_and_no_data_reports_nothing_to_remove() {
  local home
  home=$(fresh_home un-empty-purge)
  mkdir -p "$home"
  if run_uninstall "$home" --purge --yes > "$WORK/out-un-empty-purge" 2>&1; then return 1; fi
  grep -qi 'nothing to remove' "$WORK/out-un-empty-purge"
}

test_uninstall_refuses_without_a_home() {
  guard_keyring
  if env -u HOME PATH="$FAKE_BIN:$PATH" PORTHMOS_UNINSTALL_TTY="$WORK/no-terminal" sh "$UNINSTALL_SH" --purge --yes > "$WORK/out-un-nohome" 2>&1; then return 1; fi
  grep -q 'HOME' "$WORK/out-un-nohome"
}

test_uninstall_rejects_an_unknown_option_and_removes_nothing() {
  local home
  home=$(fresh_home un-badopt)
  fake_install "$home"
  if run_uninstall "$home" --everything > "$WORK/out-un-badopt" 2>&1; then return 1; fi
  [ -x "$home/.local/share/porthmos/porthmos" ]
  grep -qi 'usage' "$WORK/out-un-badopt"
}

test_uninstall_help_describes_the_options_and_exits_cleanly() {
  local home
  home=$(fresh_home un-help)
  fake_install "$home"
  run_uninstall "$home" --help > "$WORK/out-un-help" 2>&1
  grep -q -- '--purge' "$WORK/out-un-help"
  grep -q -- '--yes' "$WORK/out-un-help"
  [ -x "$home/.local/share/porthmos/porthmos" ]
}

test_uninstall_leaves_no_temp_files_behind() {
  local home
  home=$(fresh_home un-temps)
  fake_install "$home"
  rm -rf "$WORK/tmp"
  mkdir -p "$WORK/tmp"
  run_uninstall "$home" > /dev/null 2>&1
  [ -z "$(ls -A "$WORK/tmp")" ]
}

make_fake_keyring

set +e
for name in $(declare -F | awk '{print $3}' | grep '^test_'); do
  ( set -eu; "$name" ) > "$WORK/log-$name" 2>&1
  status=$?
  if [ "$status" -eq 0 ]; then
    echo "ok - ${name#test_}"
  else
    echo "FAIL - ${name#test_}"
    sed 's/^/    /' "$WORK/log-$name"
    failures=$((failures + 1))
  fi
done

[ "$failures" -eq 0 ]
