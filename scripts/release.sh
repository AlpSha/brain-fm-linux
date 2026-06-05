#!/usr/bin/env bash
# Bump the version across the project, commit, tag, and (optionally) push.
#
# Usage:
#   scripts/release.sh 0.2.0            # set an explicit version
#   scripts/release.sh patch            # 0.1.0 -> 0.1.1
#   scripts/release.sh minor            # 0.1.0 -> 0.2.0
#   scripts/release.sh major            # 0.1.0 -> 1.0.0
#   scripts/release.sh patch --push     # skip the confirmation prompt
#
# Pushing the tag triggers .github/workflows/release.yml, which creates the
# GitHub release and updates the AUR package. The PKGBUILD checksum is finalized
# by that workflow (the release tarball doesn't exist until the tag is pushed),
# so this script only bumps pkgver here.
set -euo pipefail

cd "$(dirname "$0")/.."

CARGO=src-tauri/Cargo.toml
CONF=src-tauri/tauri.conf.json
LOCK=src-tauri/Cargo.lock
PKGB=aur/PKGBUILD

die() { echo "error: $*" >&2; exit 1; }

cur=$(grep -m1 '^version' "$CARGO" | sed -E 's/.*"([0-9]+\.[0-9]+\.[0-9]+)".*/\1/')
[ -n "$cur" ] || die "could not read current version from $CARGO"

arg=${1:-}
[ -n "$arg" ] || die "usage: release.sh <version|major|minor|patch> [--push]"

case "$arg" in
  major|minor|patch)
    IFS=. read -r MA MI PA <<<"$cur"
    case "$arg" in
      major) MA=$((MA + 1)); MI=0; PA=0 ;;
      minor) MI=$((MI + 1)); PA=0 ;;
      patch) PA=$((PA + 1)) ;;
    esac
    new="$MA.$MI.$PA"
    ;;
  [0-9]*.[0-9]*.[0-9]*) new="$arg" ;;
  *) die "invalid version or bump keyword: $arg" ;;
esac

[ "$new" != "$cur" ] || die "version is already $cur"
echo "Bumping $cur -> $new"

[ -z "$(git status --porcelain)" ] || die "working tree not clean — commit or stash first"
git rev-parse "v$new" >/dev/null 2>&1 && die "tag v$new already exists"

# tauri.conf.json:  "version": "x.y.z"
sed -i -E "s/(\"version\": \")$cur(\")/\1$new\2/" "$CONF"
# Cargo.toml: first  version = "x.y.z"  (the [package] one)
sed -i -E "0,/^version = \"$cur\"/s//version = \"$new\"/" "$CARGO"
# Cargo.lock: the version line right after  name = "brain-fm"
sed -i -E "/^name = \"brain-fm\"/{n;s/^version = \".*\"/version = \"$new\"/}" "$LOCK"
# aur/PKGBUILD: pkgver + reset pkgrel (sha256sums finalized by CI on tag)
sed -i -E "s/^pkgver=.*/pkgver=$new/" "$PKGB"
sed -i -E "s/^pkgrel=.*/pkgrel=1/" "$PKGB"

echo "Updated version strings:"
grep -H '^version'   "$CARGO"
grep -H '"version"'  "$CONF"
grep -H '^pkgver'    "$PKGB"
sed -n '/^name = "brain-fm"/{n;p}' "$LOCK"

git add "$CONF" "$CARGO" "$LOCK" "$PKGB"
git commit -m "Release v$new"
git tag -a "v$new" -m "brain-fm $new"
echo "Committed and tagged v$new."

push=0
[ "${2:-}" = "--push" ] && push=1
if [ "$push" -eq 0 ]; then
  read -r -p "Push commit + tag to origin (triggers release CI + AUR deploy)? [y/N] " ans
  [[ "${ans:-}" =~ ^[Yy] ]] && push=1
fi

if [ "$push" -eq 1 ]; then
  git push origin HEAD
  git push origin "v$new"
  echo "Pushed. Watch the release: https://github.com/AlpSha/brain-fm-linux/actions"
else
  echo "Not pushed. To release later:"
  echo "  git push origin HEAD && git push origin v$new"
fi
