#!/usr/bin/env bash
# Cut a release: bump versions, regenerate CHANGELOG, commit, tag, push.
#
# Usage: scripts/release.sh X.Y.Z
#
# Requirements (install once):
#   cargo install cargo-edit     # provides `cargo set-version`
#   cargo install git-cliff      # changelog generator
#
# The frontend uses pnpm (see frontend/package.json `packageManager`). The
# script will never invoke npm or yarn.
#
# Preconditions:
#   - on branch `main`
#   - working tree clean
#   - local main is up-to-date with origin/main
set -euo pipefail

die() { echo "error: $*" >&2; exit 1; }

VERSION="${1:-}"
[[ -n "$VERSION" ]] || die "usage: $0 X.Y.Z"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] \
  || die "VERSION must match semver X.Y.Z (optionally with -prerelease), got: $VERSION"

command -v cargo     >/dev/null || die "cargo not found"
command -v pnpm      >/dev/null || die "pnpm not found"
command -v git-cliff >/dev/null || die "git-cliff not found. Install with: cargo install git-cliff"
cargo set-version --help >/dev/null 2>&1 \
  || die "cargo-edit not installed. Install with: cargo install cargo-edit"

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[[ "$BRANCH" == "main" ]] || die "must be on branch main (currently: $BRANCH)"

[[ -z "$(git status --porcelain)" ]] || die "working tree is not clean"

echo "==> Fetching origin"
git fetch origin main --tags
LOCAL="$(git rev-parse @)"
REMOTE="$(git rev-parse @{u})"
[[ "$LOCAL" == "$REMOTE" ]] || die "local main is not in sync with origin/main"

TAG="v$VERSION"
git rev-parse "$TAG" >/dev/null 2>&1 && die "tag $TAG already exists"

echo "==> Bumping Rust workspace to $VERSION"
cargo set-version --workspace "$VERSION"

echo "==> Bumping frontend (pnpm) to $VERSION"
( cd frontend && pnpm version "$VERSION" --no-git-tag-version --allow-same-version >/dev/null )

echo "==> Refreshing Cargo.lock"
cargo check --workspace --quiet

echo "==> Regenerating CHANGELOG.md"
git-cliff --tag "$TAG" --output CHANGELOG.md

echo "==> Staging changes"
git add -A
git status --short

echo
read -r -p "Create commit and tag $TAG? [y/N] " CONFIRM
[[ "$CONFIRM" =~ ^[Yy]$ ]] || die "aborted before commit"

git commit -m "chore(release): $TAG"
git tag -a "$TAG" -m "Release $TAG"

echo
read -r -p "Push main and $TAG to origin? [y/N] " CONFIRM
if [[ "$CONFIRM" =~ ^[Yy]$ ]]; then
  git push origin main
  git push origin "$TAG"
  echo "==> Pushed. Release workflow will run on tag $TAG."
else
  echo "==> Commit and tag created locally. Push manually when ready:"
  echo "    git push origin main && git push origin $TAG"
fi
