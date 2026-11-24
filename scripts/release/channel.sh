#!/bin/sh
set -eu

NUM='(0|[1-9][0-9]*)'
IDENTIFIER='(0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)'

usage() {
  cat >&2 << 'USAGE'
Usage: channel.sh parse TAG
       channel.sh check TAG COMMIT

A release tag is the window app's version with the letter G after the year and
the dash written as a dot:

  2026G.5.0          stable release of version 2026.5.0
  2026G.5.0.dev.1    preview release of version 2026.5.0-dev.1
  2026G.5.0.rc.1     candidate of version 2026.5.0-rc.1

The terminal app (letter T) is no longer released.

parse prints three lines, product=gui, channel=stable|preview|candidate and
version=<semver>, or explains why the tag is not a release tag.

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

parse_tag() {
  tag=$1
  printf '%s\n' "$tag" | grep -Eq "^[1-9][0-9]{3}G\\.$NUM\\.$NUM(\\.$IDENTIFIER(\\.$IDENTIFIER)*)?\$" || return 1

  year=${tag%%G*}
  rest=${tag#*G.}
  minor=${rest%%.*}
  rest=${rest#*.}
  patch=${rest%%.*}
  if [ "$patch" = "$rest" ]; then
    suffix=
  else
    suffix=${rest#*.}
  fi

  product=gui
  version=$year.$minor.$patch
  if [ -z "$suffix" ]; then
    channel=stable
  else
    version=$version-$suffix
    if printf '%s\n' "$suffix" | grep -Eq "^dev(\\.$NUM)?\$"; then
      channel=preview
    else
      channel=candidate
    fi
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

  case $action in
    parse | check) ;;
    *)
      usage
      exit 2
      ;;
  esac

  if printf '%s\n' "$tag" | grep -Eq '^[1-9][0-9]{3}T\.'; then
    fail "$tag names the terminal app, which is no longer released; use the window app tag letter G, for example 2026G.5.0"
  fi
  parse_tag "$tag" || fail "$tag is not a release tag: use the year, G, and the version, for example 2026G.5.0, 2026G.5.0.dev.1 or 2026G.5.0.rc.1"

  if [ "$action" = check ]; then
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
  fi

  printf 'product=%s\nchannel=%s\nversion=%s\n' "$product" "$channel" "$version"
}

main "$@"
