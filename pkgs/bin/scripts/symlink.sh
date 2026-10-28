#!/usr/bin/env bash

# This script links the development compiler into ~/.local/bin.

set -euo pipefail

# shellcheck source-path=SCRIPTDIR
source "$(dirname "${BASH_SOURCE[0]}")/_package_env.sh"

ext=""

if [[ "$(rustc -vV | sed -n 's/^host: //p')" == *windows* ]]; then
	ext=".exe"
fi

binary_path="$package_dist_dir/development/errs$ext"
link_path="$HOME/.local/bin/errs$ext"

if [[ ! -f "$binary_path" ]]; then
	echo "🔴 Build the development compiler before linking it." >&2
	exit 1
fi

if [[ -e "$link_path" && ! -L "$link_path" ]]; then
	echo "🔴 Cannot replace existing file: $link_path" >&2
	exit 1
fi

mkdir -p "$(dirname "$link_path")"
ln -sfn "$binary_path" "$link_path"

echo "🟢 Linked: $link_path -> $binary_path"
