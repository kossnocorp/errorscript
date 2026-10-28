#!/usr/bin/env bash

# This script builds and packages the VS Code extension.

set -euo pipefail

# shellcheck source-path=SCRIPTDIR
source "$(dirname "${BASH_SOURCE[0]}")/_package_env.sh"

echo -e "🚧 Packaging VS Code extension\n"

cd "$root_dir"

echo "🌀 Building VS Code extension source code..."

if ! output=$(mise :build 2>&1); then
	echo '╭─ 🔴 Failed ──────────────────────────────────────╮'
	echo "$output"
	echo '╰───────────────────────────────────────────────────╯'
	exit 1
fi

echo "🟢 Built"

echo -e "\n🌀 Packaging vsix..."

vsce_path="$(pnpm --reporter=silent -c exec which vsce)"
cd "$package_dist_dir"

if ! output=$("$vsce_path" pack \
	--no-dependencies \
	--target "$package_target" \
	--out "$package_filename" \
	--baseContentUrl https://github.com/kossnocorp/errorscript/tree/HEAD/pkgs/vscode \
	--baseImagesUrl https://github.com/kossnocorp/errorscript/raw/HEAD/pkgs/vscode \
	2>&1); then

	echo '╭─ 🔴 Failed ──────────────────────────────────────╮'
	echo "$output"
	echo '╰───────────────────────────────────────────────────╯'
	exit 1
fi

echo "🟢 Packaged: $package_path"

echo -e "\n🎉 Extension package is ready!"
