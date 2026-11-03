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

test_a_plain_version_tag_is_the_stable_channel() {
  [ "$(sh "$CHANNEL_SH" classify v0.5.0)" = stable ]
  [ "$(sh "$CHANNEL_SH" classify v10.20.30)" = stable ]
}

test_a_dev_tag_with_or_without_a_number_is_the_preview_channel() {
  [ "$(sh "$CHANNEL_SH" classify v0.5.0-dev)" = preview ]
  [ "$(sh "$CHANNEL_SH" classify v0.5.0-dev.1)" = preview ]
  [ "$(sh "$CHANNEL_SH" classify v0.5.0-dev.12)" = preview ]
}

test_any_other_pre_release_tag_is_a_candidate() {
  [ "$(sh "$CHANNEL_SH" classify v0.4.0-rc.2)" = candidate ]
  [ "$(sh "$CHANNEL_SH" classify v0.5.0-beta.1)" = candidate ]
  [ "$(sh "$CHANNEL_SH" classify v0.5.0-developer)" = candidate ]
  [ "$(sh "$CHANNEL_SH" classify v0.5.0-dev-1)" = candidate ]
  [ "$(sh "$CHANNEL_SH" classify v0.5.0-dev.1.2)" = candidate ]
}

test_a_tag_that_is_not_a_version_is_rejected() {
  for tag in preview latest v1 v1.2 1.2.3 v1.2.3.4 v1.2.x release-1 v1.2.3- "v1.2.3-dev "; do
    if sh "$CHANNEL_SH" classify "$tag" > "$WORK/out-bad" 2>&1; then
      echo "accepted $tag"
      return 1
    fi
  done
}

test_a_missing_tag_is_rejected() {
  if sh "$CHANNEL_SH" classify > "$WORK/out-missing" 2>&1; then return 1; fi
  grep -qi 'usage' "$WORK/out-missing"
}

test_a_stable_tag_on_a_commit_of_main_is_allowed() {
  local repo sha
  repo=$(make_repo stable-ok)
  sha=$(commit "$repo" "on main")
  git_in "$repo" push -q origin main
  [ "$(channel_check "$repo" v0.5.0 "$sha")" = stable ]
}

test_a_stable_tag_on_a_commit_only_on_development_is_refused() {
  local repo sha
  repo=$(make_repo stable-dev)
  git_in "$repo" checkout -q development
  sha=$(commit "$repo" "only on development")
  git_in "$repo" push -q origin development
  if channel_check "$repo" v0.5.0 "$sha" > "$WORK/out-stable-dev" 2>&1; then return 1; fi
  grep -q 'main' "$WORK/out-stable-dev"
}

test_a_dev_tag_on_a_commit_of_development_is_allowed() {
  local repo sha
  repo=$(make_repo dev-ok)
  git_in "$repo" checkout -q development
  sha=$(commit "$repo" "on development")
  git_in "$repo" push -q origin development
  [ "$(channel_check "$repo" v0.5.0-dev.1 "$sha")" = preview ]
}

test_a_dev_tag_on_a_commit_only_on_main_is_refused() {
  local repo sha
  repo=$(make_repo dev-main)
  sha=$(commit "$repo" "only on main")
  git_in "$repo" push -q origin main
  if channel_check "$repo" v0.5.0-dev.1 "$sha" > "$WORK/out-dev-main" 2>&1; then return 1; fi
  grep -q 'development' "$WORK/out-dev-main"
}

test_a_dev_tag_on_a_commit_of_a_feature_branch_is_refused() {
  local repo sha
  repo=$(make_repo dev-feature)
  git_in "$repo" checkout -q -b feature
  sha=$(commit "$repo" "feature work")
  git_in "$repo" push -q origin feature
  if channel_check "$repo" v0.5.0-dev.1 "$sha" > /dev/null 2>&1; then return 1; fi
}

test_a_commit_that_is_on_both_branches_may_be_stable_or_preview() {
  local repo sha
  repo=$(make_repo both)
  sha=$(git_in "$repo" rev-parse HEAD)
  [ "$(channel_check "$repo" v0.5.0 "$sha")" = stable ]
  [ "$(channel_check "$repo" v0.5.0-dev.2 "$sha")" = preview ]
}

test_a_candidate_tag_may_be_on_any_commit() {
  local repo sha
  repo=$(make_repo candidate)
  git_in "$repo" checkout -q -b feature
  sha=$(commit "$repo" "feature work")
  git_in "$repo" push -q origin feature
  [ "$(channel_check "$repo" v0.5.0-rc.1 "$sha")" = candidate ]
}

test_check_needs_a_tag_and_a_commit() {
  local repo
  repo=$(make_repo args)
  if (cd "$repo" && sh "$CHANNEL_SH" check v0.5.0 > "$WORK/out-args" 2>&1); then return 1; fi
  grep -qi 'usage' "$WORK/out-args"
}

test_check_refuses_an_unknown_commit() {
  local repo
  repo=$(make_repo unknown)
  if channel_check "$repo" v0.5.0 0000000000000000000000000000000000000000 > /dev/null 2>&1; then return 1; fi
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
