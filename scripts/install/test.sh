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
unset XDG_CONFIG_HOME XDG_STATE_HOME XDG_DATA_HOME PORTHMOS_INSTALL_DIR PORTHMOS_BIN_DIR

release_tool() {
  (cd "$ROOT" && cargo run -q -p porthmos-release -- "$@")
}

make_release() {
  local dir=$1 marker=$2 product=${3:-tui} icon=${4:-icon} program=porthmos
  local icon_flags=()
  [ "$product" = gui ] && program=porthmos-gui
  mkdir -p "$dir"
  printf '#!/bin/sh\necho "%s-stub %s"\n' "$program" "$marker" > "$dir/$program"
  chmod 755 "$dir/$program"
  printf 'MIT\n' > "$dir/LICENSE"
  if [ "$product" = gui ] && [ "$icon" = icon ]; then
    printf '<svg>%s</svg>\n' "$marker" > "$dir/icon.svg"
    icon_flags=(--icon "$dir/icon.svg")
  fi
  release_tool package --product "$product" --target "$TARGET" --version 2026.5.0 --binary "$dir/$program" --license "$dir/LICENSE" "${icon_flags[@]}" --out "$dir/dist" > /dev/null
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

shadow_curl_serving_a_tampered_archive() {
  local tampered=$1 times=$2 real
  real=$(command -v curl)
  mkdir -p "$FAKE_BIN"
  printf '%s\n' 0 > "$WORK/curl-served"
  cat > "$FAKE_BIN/curl" << SHADOW
#!/bin/sh
$real "\$@" || exit \$?
out=
url=
while [ \$# -gt 0 ]; do
  case \$1 in
    -o) out=\$2; shift ;;
    http*|file*) url=\$1 ;;
  esac
  shift
done
case \$url in
  *.tar.gz)
    served=\$(cat "$WORK/curl-served")
    if [ "\$served" -lt $times ]; then
      cp "$tampered" "\$out"
      echo \$((served + 1)) > "$WORK/curl-served"
    fi
    ;;
esac
SHADOW
  chmod 755 "$FAKE_BIN/curl"
}

run_install_with_shadowed_curl() {
  local home=$1 base=$2
  HOME="$home" PATH="$FAKE_BIN:$PATH" PORTHMOS_RELEASE_BASE="$base" PORTHMOS_TARGET="$TARGET" PORTHMOS_RETRY_DELAY=0 TMPDIR="$WORK/tmp" sh "$INSTALL_SH"
}

test_an_archive_caught_between_two_uploads_is_fetched_again() {
  local home
  home=$(fresh_home between-uploads)
  make_release "$WORK/rel-between" one
  make_release "$WORK/rel-between-bad" tampered
  shadow_curl_serving_a_tampered_archive "$WORK/rel-between-bad/dist/porthmos-$TARGET.tar.gz" 1
  run_install_with_shadowed_curl "$home" "file://$WORK/rel-between/dist" > "$WORK/out-between" 2>&1
  rm -f "$FAKE_BIN/curl"
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub one" ]
  [ "$(cat "$WORK/curl-served")" = 1 ]
}

test_an_archive_that_never_matches_its_checksum_still_installs_nothing() {
  local home
  home=$(fresh_home never-matches)
  make_release "$WORK/rel-never" one
  make_release "$WORK/rel-never-bad" tampered
  shadow_curl_serving_a_tampered_archive "$WORK/rel-never-bad/dist/porthmos-$TARGET.tar.gz" 99
  if run_install_with_shadowed_curl "$home" "file://$WORK/rel-never/dist" > "$WORK/out-never" 2>&1; then
    rm -f "$FAKE_BIN/curl"
    return 1
  fi
  rm -f "$FAKE_BIN/curl"
  grep -q 'does not match its checksum; nothing was installed' "$WORK/out-never"
  [ ! -e "$home/.local/bin/porthmos" ]
  [ ! -e "$home/.local/share/porthmos/porthmos" ]
  [ "$(cat "$WORK/curl-served")" = 2 ]
}

test_every_app_and_channel_has_its_own_default_location() {
  (
    export PORTHMOS_INSTALL_SOURCE_ONLY=1
    # shellcheck disable=SC1090
    . "$INSTALL_SH"
    [ "$(release_base tui stable)" = "https://github.com/Smyrnis/porthmos/releases/download/tui-stable" ]
    [ "$(release_base tui preview)" = "https://github.com/Smyrnis/porthmos/releases/download/tui-preview" ]
    [ "$(release_base gui stable)" = "https://github.com/Smyrnis/porthmos/releases/download/gui-stable" ]
    [ "$(release_base gui preview)" = "https://github.com/Smyrnis/porthmos/releases/download/gui-preview" ]
  )
}

test_each_channel_location_can_be_overridden_on_its_own() {
  (
    export PORTHMOS_INSTALL_SOURCE_ONLY=1
    # shellcheck disable=SC1090
    . "$INSTALL_SH"
    export PORTHMOS_RELEASE_BASE=https://stable.example/x
    export PORTHMOS_PREVIEW_BASE=https://preview.example/y
    [ "$(release_base tui stable)" = https://stable.example/x ]
    [ "$(release_base gui stable)" = https://stable.example/x ]
    [ "$(release_base tui preview)" = https://preview.example/y ]
    [ "$(release_base gui preview)" = https://preview.example/y ]
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

test_the_gui_flag_installs_the_gui_and_leaves_the_tui_alone() {
  local home
  home=$(fresh_home gui-alone)
  make_release "$WORK/rel-gui-alone-tui" tui-build tui
  make_release "$WORK/rel-gui-alone-gui" gui-build gui
  run_install_channel "$home" "file://$WORK/rel-gui-alone-tui/dist" "file://$WORK/none" > /dev/null 2>&1
  run_install_channel "$home" "file://$WORK/rel-gui-alone-gui/dist" "file://$WORK/none" --gui > "$WORK/out-gui-alone" 2>&1
  [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub gui-build" ]
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub tui-build" ]
  [ -L "$home/.local/bin/porthmos-gui" ]
  [ "$(readlink "$home/.local/bin/porthmos-gui")" = "$home/.local/share/porthmos/porthmos-gui" ]
  grep -q 'porthmos-gui' "$WORK/out-gui-alone"
}

test_the_gui_and_preview_flags_in_either_order_install_the_gui_preview_build() {
  local home order
  make_release "$WORK/rel-gp-stable" stable-gui gui
  make_release "$WORK/rel-gp-preview" preview-gui gui
  for order in "--gui --preview" "--preview --gui"; do
    home=$(fresh_home "gp-${order// /}")
    # shellcheck disable=SC2086
    run_install_channel "$home" "file://$WORK/rel-gp-stable/dist" "file://$WORK/rel-gp-preview/dist" $order > "$WORK/out-gp" 2>&1
    [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub preview-gui" ]
    grep -qi 'preview' "$WORK/out-gp"
    [ ! -e "$home/.local/bin/porthmos" ]
  done
}

test_the_gui_flag_alone_uses_the_stable_location() {
  local home
  home=$(fresh_home gui-stable)
  make_release "$WORK/rel-gs-stable" stable-gui gui
  make_release "$WORK/rel-gs-preview" preview-gui gui
  run_install_channel "$home" "file://$WORK/rel-gs-stable/dist" "file://$WORK/rel-gs-preview/dist" --gui > "$WORK/out-gs" 2>&1
  [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub stable-gui" ]
  ! grep -qi 'preview' "$WORK/out-gs"
}

test_the_gui_flag_against_a_location_with_only_the_tui_installs_nothing() {
  local home
  home=$(fresh_home gui-wrong)
  make_release "$WORK/rel-gw-tui" tui-build tui
  if run_install_channel "$home" "file://$WORK/rel-gw-tui/dist" "file://$WORK/none" --gui > "$WORK/out-gw" 2>&1; then return 1; fi
  grep -q 'porthmos-gui-' "$WORK/out-gw"
  [ ! -e "$home/.local/bin/porthmos-gui" ]
  [ ! -e "$home/.local/share/porthmos/porthmos-gui" ]
}

test_installing_the_gui_again_replaces_it_and_never_touches_the_tui() {
  local home
  home=$(fresh_home gui-again)
  make_release "$WORK/rel-ga-tui" tui-build tui
  make_release "$WORK/rel-ga-one" gui-one gui
  make_release "$WORK/rel-ga-two" gui-two gui
  run_install_channel "$home" "file://$WORK/rel-ga-tui/dist" "file://$WORK/none" > /dev/null 2>&1
  run_install_channel "$home" "file://$WORK/rel-ga-one/dist" "file://$WORK/none" --gui > /dev/null 2>&1
  run_install_channel "$home" "file://$WORK/rel-ga-two/dist" "file://$WORK/none" --gui > /dev/null 2>&1
  [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub gui-two" ]
  run_install_channel "$home" "file://$WORK/rel-ga-tui/dist" "file://$WORK/none" > /dev/null 2>&1
  [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub gui-two" ]
  [ "$("$home/.local/bin/porthmos")" = "porthmos-stub tui-build" ]
}

test_install_help_describes_the_gui_flag() {
  local home
  home=$(fresh_home gui-help)
  mkdir -p "$home"
  run_install_channel "$home" "file://$WORK/none" "file://$WORK/none" --help > "$WORK/out-gui-help" 2>&1
  grep -q -- '--gui' "$WORK/out-gui-help"
  grep -q -- '--preview' "$WORK/out-gui-help"
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

fake_install_gui() {
  local home=$1
  mkdir -p "$home/.local/share/porthmos" "$home/.local/bin"
  printf '#!/bin/sh\necho stub\n' > "$home/.local/share/porthmos/porthmos-gui"
  chmod 755 "$home/.local/share/porthmos/porthmos-gui"
  ln -s "$home/.local/share/porthmos/porthmos-gui" "$home/.local/bin/porthmos-gui"
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

test_uninstall_removes_a_lone_gui_with_its_link_and_the_empty_folder() {
  local home
  home=$(fresh_home un-gui)
  fake_install_gui "$home"
  run_uninstall "$home" > "$WORK/out-un-gui" 2>&1
  [ ! -e "$home/.local/share/porthmos/porthmos-gui" ]
  [ ! -L "$home/.local/bin/porthmos-gui" ]
  [ ! -e "$home/.local/share/porthmos" ]
  grep -q "$home/.local/bin/porthmos-gui" "$WORK/out-un-gui"
}

test_uninstall_removes_both_apps_when_both_are_installed() {
  local home
  home=$(fresh_home un-both)
  fake_install "$home"
  fake_install_gui "$home"
  run_uninstall "$home" > "$WORK/out-un-both" 2>&1
  [ ! -e "$home/.local/share/porthmos" ]
  [ ! -L "$home/.local/bin/porthmos" ]
  [ ! -L "$home/.local/bin/porthmos-gui" ]
}

test_uninstall_leaves_a_gui_link_that_points_somewhere_else_and_still_removes_the_tui() {
  local home
  home=$(fresh_home un-gui-otherlink)
  fake_install "$home"
  mkdir -p "$home/.local/bin"
  ln -s /bin/sh "$home/.local/bin/porthmos-gui"
  run_uninstall "$home" > "$WORK/out-un-gui-otherlink" 2>&1
  [ "$(readlink "$home/.local/bin/porthmos-gui")" = /bin/sh ]
  [ ! -L "$home/.local/bin/porthmos" ]
  [ ! -e "$home/.local/share/porthmos/porthmos" ]
  grep -q "$home/.local/bin/porthmos-gui" "$WORK/out-un-gui-otherlink"
}

test_uninstall_leaves_a_regular_file_named_like_the_gui() {
  local home
  home=$(fresh_home un-gui-file)
  fake_install "$home"
  printf 'mine\n' > "$home/.local/bin/porthmos-gui"
  run_uninstall "$home" > "$WORK/out-un-gui-file" 2>&1
  [ "$(cat "$home/.local/bin/porthmos-gui")" = mine ]
  [ ! -L "$home/.local/bin/porthmos" ]
}

test_uninstall_keeps_the_install_folder_while_a_file_that_is_not_ours_is_in_it() {
  local home
  home=$(fresh_home un-gui-foreign)
  fake_install_gui "$home"
  printf 'x\n' > "$home/.local/share/porthmos/notes.txt"
  run_uninstall "$home" > "$WORK/out-un-gui-foreign" 2>&1
  [ -f "$home/.local/share/porthmos/notes.txt" ]
  [ ! -e "$home/.local/share/porthmos/porthmos-gui" ]
}

test_purge_with_only_the_gui_installed_removes_its_data_too() {
  local home
  home=$(fresh_home un-gui-purge)
  fake_install_gui "$home"
  fake_data "$home"
  run_uninstall "$home" --purge --yes > "$WORK/out-un-gui-purge" 2>&1
  [ ! -e "$home/.local/share/porthmos" ]
  [ ! -e "$home/.config/porthmos" ]
  [ -f "$home/.config/other/keep.toml" ]
  [ -f "$home/.ssh/known_hosts" ]
}

test_uninstall_without_purge_keeps_settings_when_removing_the_gui() {
  local home
  home=$(fresh_home un-gui-keep)
  fake_install_gui "$home"
  fake_data "$home"
  run_uninstall "$home" > "$WORK/out-un-gui-keep" 2>&1
  [ -f "$home/.config/porthmos/connections.toml" ]
  [ -f "$home/.local/state/porthmos/state.toml" ]
}

test_uninstall_removes_the_previous_versions_an_update_left_behind() {
  local home
  home=$(fresh_home un-old)
  fake_install "$home"
  fake_install_gui "$home"
  printf 'old\n' > "$home/.local/share/porthmos/porthmos.old"
  printf 'old\n' > "$home/.local/share/porthmos/porthmos-gui.old"
  run_uninstall "$home" > "$WORK/out-un-old" 2>&1
  [ ! -e "$home/.local/share/porthmos" ]
}

test_uninstall_does_not_remove_other_files_that_end_in_old() {
  local home
  home=$(fresh_home un-old-other)
  fake_install "$home"
  printf 'mine\n' > "$home/.local/share/porthmos/notes.old"
  run_uninstall "$home" > "$WORK/out-un-old-other" 2>&1
  [ -f "$home/.local/share/porthmos/notes.old" ]
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

desktop_file() {
  printf '%s\n' "${1:-$HOME}/.local/share/applications/porthmos-gui.desktop"
}

icon_file() {
  printf '%s\n' "$1/.local/share/icons/hicolor/scalable/apps/porthmos-gui.svg"
}

install_gui() {
  local home=$1 base=$2
  run_install_channel "$home" "$base" "$base" --gui
}

test_the_window_app_gets_a_launcher_and_an_icon() {
  local home desktop
  home=$(fresh_home launcher)
  make_release "$WORK/rel-launcher" one gui
  install_gui "$home" "file://$WORK/rel-launcher/dist" > "$WORK/out-launcher" 2>&1
  desktop=$(desktop_file "$home")
  [ -f "$desktop" ]
  grep -qx '\[Desktop Entry\]' "$desktop"
  grep -qx 'Type=Application' "$desktop"
  grep -qx 'Name=Porthmos' "$desktop"
  grep -qx 'Terminal=false' "$desktop"
  grep -qx 'Icon=porthmos-gui' "$desktop"
  grep -qx 'StartupWMClass=porthmos-gui' "$desktop"
  grep -qx "Exec=\"$home/.local/share/porthmos/porthmos-gui\"" "$desktop"
  grep -q '^Categories=.*Network' "$desktop"
  [ "$(cat "$(icon_file "$home")")" = "<svg>one</svg>" ]
}

test_the_launcher_follows_xdg_data_home() {
  local home data
  home=$(fresh_home launcher-xdg)
  data="$WORK/xdg-data"
  make_release "$WORK/rel-launcher-xdg" one gui
  (
    export XDG_DATA_HOME="$data"
    install_gui "$home" "file://$WORK/rel-launcher-xdg/dist" > "$WORK/out-launcher-xdg" 2>&1
  )
  [ -f "$data/applications/porthmos-gui.desktop" ]
  [ -f "$data/icons/hicolor/scalable/apps/porthmos-gui.svg" ]
  [ ! -e "$home/.local/share/applications/porthmos-gui.desktop" ]
}

test_the_launcher_quotes_an_install_path_with_a_space() {
  local home
  home="$WORK/home with space"
  mkdir -p "$WORK/tmp"
  make_release "$WORK/rel-launcher-space" one gui
  install_gui "$home" "file://$WORK/rel-launcher-space/dist" > "$WORK/out-launcher-space" 2>&1
  grep -qxF "Exec=\"$home/.local/share/porthmos/porthmos-gui\"" "$(desktop_file "$home")"
}

test_the_terminal_app_gets_no_launcher() {
  local home
  home=$(fresh_home launcher-tui)
  make_release "$WORK/rel-launcher-tui" one
  run_install "$home" "file://$WORK/rel-launcher-tui/dist" > /dev/null 2>&1
  [ ! -e "$home/.local/share/applications" ]
  [ ! -e "$home/.local/share/icons" ]
}

test_an_archive_without_an_icon_installs_without_a_launcher() {
  local home
  home=$(fresh_home launcher-noicon)
  make_release "$WORK/rel-launcher-noicon" one gui noicon
  install_gui "$home" "file://$WORK/rel-launcher-noicon/dist" > "$WORK/out-launcher-noicon" 2>&1
  [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub one" ]
  [ ! -e "$(desktop_file "$home")" ]
}

test_macos_gets_no_launcher() {
  local home bin
  home=$(fresh_home launcher-mac)
  bin="$WORK/darwin-bin"
  mkdir -p "$bin"
  printf '#!/bin/sh\necho Darwin\n' > "$bin/uname"
  chmod 755 "$bin/uname"
  make_release "$WORK/rel-launcher-mac" one gui
  (
    export PATH="$bin:$PATH"
    install_gui "$home" "file://$WORK/rel-launcher-mac/dist" > "$WORK/out-launcher-mac" 2>&1
  )
  [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub one" ]
  [ ! -e "$home/.local/share/applications" ]
}

test_a_launcher_that_cannot_be_written_does_not_fail_the_install() {
  local home
  home=$(fresh_home launcher-blocked)
  mkdir -p "$home/.local/share"
  printf 'in the way\n' > "$home/.local/share/applications"
  make_release "$WORK/rel-launcher-blocked" one gui
  install_gui "$home" "file://$WORK/rel-launcher-blocked/dist" > "$WORK/out-launcher-blocked" 2>&1
  [ "$("$home/.local/bin/porthmos-gui")" = "porthmos-gui-stub one" ]
  grep -q "Couldn't add the launcher" "$WORK/out-launcher-blocked"
}

test_installing_again_keeps_one_launcher_that_points_at_the_program() {
  local home
  home=$(fresh_home launcher-again)
  make_release "$WORK/rel-launcher-again-1" one gui
  make_release "$WORK/rel-launcher-again-2" two gui
  install_gui "$home" "file://$WORK/rel-launcher-again-1/dist" > /dev/null 2>&1
  install_gui "$home" "file://$WORK/rel-launcher-again-2/dist" > /dev/null 2>&1
  [ "$(grep -c '^Exec=' "$(desktop_file "$home")")" = 1 ]
  [ "$(cat "$(icon_file "$home")")" = "<svg>two</svg>" ]
}

test_uninstall_removes_the_launcher_and_the_icon() {
  local home other
  home=$(fresh_home un-launcher)
  make_release "$WORK/rel-un-launcher" one gui
  install_gui "$home" "file://$WORK/rel-un-launcher/dist" > /dev/null 2>&1
  other="$home/.local/share/applications/other.desktop"
  printf '[Desktop Entry]\nName=Other\n' > "$other"
  run_uninstall "$home" > "$WORK/out-un-launcher" 2>&1
  [ ! -e "$(desktop_file "$home")" ]
  [ ! -e "$(icon_file "$home")" ]
  [ -f "$other" ]
}

test_uninstall_leaves_a_launcher_of_the_same_name_that_it_did_not_write() {
  local home desktop
  home=$(fresh_home un-foreign)
  fake_install_gui "$home"
  desktop=$(desktop_file "$home")
  mkdir -p "$(dirname "$desktop")"
  printf '[Desktop Entry]\nName=Mine\nExec=/opt/mine\n' > "$desktop"
  run_uninstall "$home" > "$WORK/out-un-foreign" 2>&1
  [ -f "$desktop" ]
  grep -qx 'Name=Mine' "$desktop"
}

test_uninstall_removes_the_launcher_with_only_basic_tools_on_the_path() {
  local home bin
  home=$(fresh_home un-launcher-bare)
  make_release "$WORK/rel-un-launcher-bare" one gui
  install_gui "$home" "file://$WORK/rel-un-launcher-bare/dist" > /dev/null 2>&1
  [ -f "$(desktop_file "$home")" ]
  [ -f "$(icon_file "$home")" ]
  bin=$(bare_path)
  HOME="$home" PATH="$bin" PORTHMOS_UNINSTALL_TTY="$WORK/no-terminal" TMPDIR="$WORK/tmp" /bin/sh "$UNINSTALL_SH" > "$WORK/out-un-launcher-bare" 2>&1
  [ ! -e "$(desktop_file "$home")" ]
  [ ! -e "$(icon_file "$home")" ]
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
