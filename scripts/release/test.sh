#!/usr/bin/env bash
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CHANNEL_SH="$ROOT/scripts/release/channel.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
failures=0

git_in() {
  local dir=$1
  shift
  git -C "$dir" -c user.name=test -c user.email=test@example.invalid -c commit.gpgsign=false "$@"
}

commit() {
  local dir=$1 message=$2
  git_in "$dir" commit -q --allow-empty -m "$message"
  git_in "$dir" rev-parse HEAD
}

make_repo() {
  local name=$1 origin="$WORK/$1-origin.git" clone="$WORK/$1"
  git init -q --bare -b main "$origin"
  git init -q -b main "$clone"
  git_in "$clone" remote add origin "$origin"
  commit "$clone" base > /dev/null
  git_in "$clone" push -q origin main
  git_in "$clone" checkout -q -b development
  git_in "$clone" push -q origin development
  git_in "$clone" checkout -q main
  printf '%s\n' "$clone"
}

channel_check() {
  local repo=$1 tag=$2 sha=$3
  (cd "$repo" && sh "$CHANNEL_SH" check "$tag" "$sha")
}

parsed() {
  sh "$CHANNEL_SH" parse "$1" | tr '\n' ' ' | sed 's/ $//'
}

test_a_calendar_tag_is_a_stable_release_of_its_app() {
  [ "$(parsed 2026T.5.0)" = "product=tui channel=stable version=2026.5.0" ]
  [ "$(parsed 2026G.5.0)" = "product=gui channel=stable version=2026.5.0" ]
  [ "$(parsed 2027T.1.0)" = "product=tui channel=stable version=2027.1.0" ]
  [ "$(parsed 2026G.12.30)" = "product=gui channel=stable version=2026.12.30" ]
}

test_a_dev_tag_with_or_without_a_number_is_a_preview_release() {
  [ "$(parsed 2026T.5.0.dev)" = "product=tui channel=preview version=2026.5.0-dev" ]
  [ "$(parsed 2026T.5.0.dev.1)" = "product=tui channel=preview version=2026.5.0-dev.1" ]
  [ "$(parsed 2026G.12.3.dev.14)" = "product=gui channel=preview version=2026.12.3-dev.14" ]
}

test_any_other_suffix_is_a_candidate() {
  [ "$(parsed 2026T.5.0.rc.1)" = "product=tui channel=candidate version=2026.5.0-rc.1" ]
  [ "$(parsed 2026G.5.0.beta)" = "product=gui channel=candidate version=2026.5.0-beta" ]
  [ "$(parsed 2026T.5.0.developer)" = "product=tui channel=candidate version=2026.5.0-developer" ]
  [ "$(parsed 2026T.5.0.dev.1.2)" = "product=tui channel=candidate version=2026.5.0-dev.1.2" ]
  [ "$(parsed 2026T.5.0.dev.x)" = "product=tui channel=candidate version=2026.5.0-dev.x" ]
  [ "$(parsed 2026T.5.0.dev-1)" = "product=tui channel=candidate version=2026.5.0-dev-1" ]
}

test_a_malformed_tag_is_rejected() {
  for tag in v0.5.0 v0.5.0-dev.1 2026.5.0 2026t.5.0 26T.5.0 02026T.5.0 20260T.5.0 2026X.5.0 2026T 2026T. 2026T.5 \
    2026T.05.0 2026T.5.00 2026T.5.0. 2026T.5.0.dev.01 2026T.5.0.rc..1 2026T.5.0-dev.1 \
    "2026T.5.0 " " 2026T.5.0" 2026T.5.0.dev.1.01 preview latest ""; do
    if sh "$CHANNEL_SH" parse "$tag" > "$WORK/out-bad" 2>&1; then
      echo "accepted '$tag'"
      return 1
    fi
    grep -q '^channel: ' "$WORK/out-bad" || { echo "no reason given for '$tag'"; return 1; }
  done
}

test_a_rejected_tag_prints_nothing_on_stdout() {
  [ -z "$(sh "$CHANNEL_SH" parse v0.5.0 2> /dev/null || true)" ]
}

test_a_missing_tag_prints_the_usage() {
  if sh "$CHANNEL_SH" parse > "$WORK/out-missing" 2>&1; then return 1; fi
  grep -qi 'usage' "$WORK/out-missing"
}

test_the_old_classify_command_is_gone() {
  if sh "$CHANNEL_SH" classify 2026T.5.0 > "$WORK/out-classify" 2>&1; then return 1; fi
  grep -qi 'usage' "$WORK/out-classify"
}

test_a_stable_tag_on_a_commit_of_main_is_allowed() {
  local repo sha
  repo=$(make_repo stable-ok)
  sha=$(commit "$repo" "on main")
  git_in "$repo" push -q origin main
  [ "$(channel_check "$repo" 2026T.5.0 "$sha" | tr '\n' ' ')" = "product=tui channel=stable version=2026.5.0 " ]
  [ "$(channel_check "$repo" 2026G.5.0 "$sha" | sed -n 1p)" = "product=gui" ]
}

test_a_stable_tag_on_a_commit_only_on_development_is_refused() {
  local repo sha
  repo=$(make_repo stable-dev)
  git_in "$repo" checkout -q development
  sha=$(commit "$repo" "only on development")
  git_in "$repo" push -q origin development
  if channel_check "$repo" 2026T.5.0 "$sha" > "$WORK/out-stable-dev" 2>/dev/null; then return 1; fi
  [ ! -s "$WORK/out-stable-dev" ]
  channel_check "$repo" 2026G.5.0 "$sha" 2>&1 | grep -q 'main'
}

test_a_dev_tag_on_a_commit_of_development_is_allowed() {
  local repo sha
  repo=$(make_repo dev-ok)
  git_in "$repo" checkout -q development
  sha=$(commit "$repo" "on development")
  git_in "$repo" push -q origin development
  [ "$(channel_check "$repo" 2026T.5.0.dev.1 "$sha" | sed -n 2p)" = "channel=preview" ]
  [ "$(channel_check "$repo" 2026G.5.0.dev.1 "$sha" | sed -n 3p)" = "version=2026.5.0-dev.1" ]
}

test_a_dev_tag_on_a_commit_only_on_main_is_refused() {
  local repo sha
  repo=$(make_repo dev-main)
  sha=$(commit "$repo" "only on main")
  git_in "$repo" push -q origin main
  if channel_check "$repo" 2026T.5.0.dev.1 "$sha" > "$WORK/out-dev-main" 2>&1; then return 1; fi
  grep -q 'development' "$WORK/out-dev-main"
}

test_a_dev_tag_on_a_commit_of_a_feature_branch_is_refused() {
  local repo sha
  repo=$(make_repo dev-feature)
  git_in "$repo" checkout -q -b feature
  sha=$(commit "$repo" "feature work")
  git_in "$repo" push -q origin feature
  if channel_check "$repo" 2026T.5.0.dev.1 "$sha" > /dev/null 2>&1; then return 1; fi
}

test_a_commit_that_is_on_both_branches_may_be_stable_or_preview() {
  local repo sha
  repo=$(make_repo both)
  sha=$(git_in "$repo" rev-parse HEAD)
  [ "$(channel_check "$repo" 2026T.5.0 "$sha" | sed -n 2p)" = "channel=stable" ]
  [ "$(channel_check "$repo" 2026T.5.0.dev.2 "$sha" | sed -n 2p)" = "channel=preview" ]
}

test_a_candidate_tag_may_be_on_any_commit() {
  local repo sha
  repo=$(make_repo candidate)
  git_in "$repo" checkout -q -b feature
  sha=$(commit "$repo" "feature work")
  git_in "$repo" push -q origin feature
  [ "$(channel_check "$repo" 2026T.5.0.rc.1 "$sha" | sed -n 2p)" = "channel=candidate" ]
}

test_check_rejects_a_malformed_tag_before_looking_at_git() {
  local repo
  repo=$(make_repo malformed)
  if channel_check "$repo" v0.5.0 "$(git_in "$repo" rev-parse HEAD)" > "$WORK/out-malformed" 2>&1; then return 1; fi
  grep -q 'not a release tag' "$WORK/out-malformed"
}

test_check_needs_a_tag_and_a_commit() {
  local repo
  repo=$(make_repo args)
  if (cd "$repo" && sh "$CHANNEL_SH" check 2026T.5.0 > "$WORK/out-args" 2>&1); then return 1; fi
  grep -qi 'usage' "$WORK/out-args"
}

test_check_refuses_an_unknown_commit() {
  local repo
  repo=$(make_repo unknown)
  if channel_check "$repo" 2026T.5.0 0000000000000000000000000000000000000000 > "$WORK/out-unknown" 2>&1; then return 1; fi
  grep -q 'not known here' "$WORK/out-unknown"
}

test_check_says_when_the_branch_was_not_fetched() {
  local repo sha
  repo=$(make_repo unfetched)
  sha=$(git_in "$repo" rev-parse HEAD)
  git_in "$repo" update-ref -d refs/remotes/origin/main
  if channel_check "$repo" 2026T.5.0 "$sha" > "$WORK/out-unfetched" 2>&1; then return 1; fi
  grep -q 'was not fetched' "$WORK/out-unfetched"
}

test_the_apps_carry_calendar_versions_and_the_gui_has_its_own() {
  local metadata
  metadata=$(cd "$ROOT" && cargo metadata --no-deps --format-version 1 --offline)
  printf '%s' "$metadata" | python3 -c '
import json, sys
packages = {p["name"]: p["version"] for p in json.load(sys.stdin)["packages"]}
tui, core, gui = packages["porthmos-tui"], packages["porthmos-core"], packages["porthmos-gui"]
assert tui == core, (tui, core)
for name, version in (("porthmos-tui", tui), ("porthmos-gui", gui)):
    assert int(version.split(".")[0]) >= 2026, (name, version)
'
  grep -Eq '^version = "[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?"$' "$ROOT/porthmos-gui/Cargo.toml"
  ! grep -q '^version.workspace = true' "$ROOT/porthmos-gui/Cargo.toml"
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
