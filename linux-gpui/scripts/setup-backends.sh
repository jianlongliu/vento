#!/usr/bin/env bash
# Set up everything the C/Rust backends need beyond cargo:
#   - the IronRDP and ryll submodules (patched upstream sources), and
#   - the prebuilt C backend libraries from the upstream release.
#
# Run once after cloning, and again after `cargo clean`.
#   ./scripts/setup-backends.sh [debug|release]
set -euo pipefail

profile="${1:-debug}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

git submodule update --init --depth 1 third_party/ironrdp third_party/ryll
./patch.sh -n ironrdp ryll
"$root/linux-gpui/scripts/fetch-backends.sh" "$profile"
