#!/usr/bin/env bash
# shellcheck disable=SC2030,SC2031
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
INSTALL_SH="$ROOT/scripts/install/install.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
TARGET=x86_64-unknown-linux-gnu
failures=0

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
