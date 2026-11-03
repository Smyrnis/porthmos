#!/bin/sh
set -eu

usage() {
  cat >&2 << 'USAGE'
Usage: channel.sh classify TAG
       channel.sh check TAG COMMIT

classify prints the channel a release tag belongs to:
  stable     v1.2.3
  preview    v1.2.3-dev or v1.2.3-dev.N
  candidate  any other pre-release, such as v1.2.3-rc.1

check does the same and also requires COMMIT to be on the branch that channel
is built from: main for stable, development for preview. A candidate may be on
any commit. It reads the branches from the remote named by PORTHMOS_REMOTE
(default origin).
USAGE
}

fail() {
  printf 'channel: %s\n' "$*" >&2
  exit 1
}

classify() {
  if printf '%s\n' "$1" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo stable
  elif printf '%s\n' "$1" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+-dev(\.[0-9]+)?$'; then
    echo preview
  elif printf '%s\n' "$1" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z][0-9A-Za-z.-]*$'; then
    echo candidate
  else
    return 1
  fi
}

branch_for() {
  case $1 in
    stable) echo main ;;
    preview) echo development ;;
    *) echo ;;
  esac
}

main() {
  [ $# -ge 2 ] || {
    usage
    exit 2
  }
  action=$1
  tag=$2

  channel=$(classify "$tag") || fail "$tag is not a release tag: use vX.Y.Z, vX.Y.Z-dev.N or another pre-release such as vX.Y.Z-rc.N"

  case $action in
    classify)
      echo "$channel"
      ;;
    check)
      [ $# -ge 3 ] || {
        usage
        exit 2
      }
      commit=$3
      branch=$(branch_for "$channel")
      if [ -n "$branch" ]; then
        remote=${PORTHMOS_REMOTE:-origin}
        git rev-parse --verify --quiet "$commit^{commit}" > /dev/null || fail "commit $commit is not known here"
        git rev-parse --verify --quiet "refs/remotes/$remote/$branch" > /dev/null || fail "branch $branch was not fetched from $remote"
        git merge-base --is-ancestor "$commit" "refs/remotes/$remote/$branch" ||
          fail "tag $tag is a $channel release, which is built from $branch, but commit $commit is not on $branch"
      fi
      echo "$channel"
      ;;
    *)
      usage
      exit 2
      ;;
  esac
}

main "$@"
