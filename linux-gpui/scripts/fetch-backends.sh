#!/usr/bin/env bash
# Fetch the prebuilt C backend libraries from the upstream Vento release and
# install them beside the binary, where the rpath ($ORIGIN/lib) finds them.
#
#   ./scripts/fetch-backends.sh [debug|release]
#
# Set VENTO_RELEASE to pin a different upstream tag.
set -euo pipefail

profile="${1:-debug}"
tag="${VENTO_RELEASE:-v8}"
root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/target/$profile/lib"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

curl -fL "https://github.com/pgaskin/vento/releases/download/$tag/vento-x86_64-linux-gnu.tar.gz" \
  -o "$tmp/vento.tar.gz"
tar xzf "$tmp/vento.tar.gz" -C "$tmp"

mkdir -p "$dest"
cp "$tmp/vento/lib/libvento_libvnc.so" \
   "$tmp/vento/lib/libvento_tigervnc.so" \
   "$tmp/vento/lib/libvento_freerdp.so" \
   "$dest/"

echo "backend libraries installed in $dest"
