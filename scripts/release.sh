#!/usr/bin/env bash
# Release cardthing: bump the version, push main, tag, publish to crates.io,
# then point the homebrew tap formula at the new tag.
#
#   scripts/release.sh 0.1.2
set -euo pipefail

version="${1:-}"
tap="${TAP_DIR:-$HOME/code/github/markryall/homebrew-tap}"
formula="$tap/Formula/cardthing.rb"
tag="v$version"

die() { echo "release: $*" >&2; exit 1; }

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "usage: $0 X.Y.Z"
cd "$(dirname "$0")/.."

current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[[ "$version" != "$current" ]] || die "already at $current"
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "tag $tag already exists"
[[ -f "$formula" ]] || die "no formula at $formula"

# the release commit should hold only the version bump, so start from an empty @ on main
for repo in . "$tap"; do
  [[ -z "$(jj -R "$repo" diff --summary)" ]] || die "$repo has uncommitted changes in @"
  [[ "$(jj -R "$repo" log --no-graph -r '@-' -T 'bookmarks')" == *main* ]] || die "$repo: @ is not on main"
done

cargo test --quiet

sed -i '' "s/^version = \"$current\"/version = \"$version\"/" Cargo.toml
cargo check --quiet # refreshes Cargo.lock
cargo publish --dry-run --allow-dirty --quiet

jj describe -m "release $tag"
fish -c push

commit=$(jj log --no-graph -r main -T commit_id)
git tag "$tag" "$commit"
git push origin "$tag"

cargo publish --quiet

tarball="https://github.com/markryall/cardthing/archive/refs/tags/$tag.tar.gz"
sha=""
for _ in 1 2 3 4 5; do
  if sha=$(curl -fsL "$tarball" | shasum -a 256 | cut -d' ' -f1); then break; fi
  sha=""
  sleep 3
done
[[ -n "$sha" ]] || die "could not download $tarball"

sed -i '' \
  -e "s|^  url \".*\"|  url \"$tarball\"|" \
  -e "s|^  sha256 \".*\"|  sha256 \"$sha\"|" \
  "$formula"
jj -R "$tap" describe -m "cardthing $version"
(cd "$tap" && fish -c push)

echo "released cardthing $version 💅"
