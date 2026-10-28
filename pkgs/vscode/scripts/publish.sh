#!/usr/bin/env bash

# Publish to Visual Studio Marketplace and Open VSX using VSCE_PAT and OVSX_PAT.

set -euo pipefail

# shellcheck source-path=SCRIPTDIR
source "$(dirname "${BASH_SOURCE[0]}")/_package_env.sh"

cd "$root_dir"

if [[ -n "${GITHUB_REF_NAME:-}" && "$GITHUB_REF_NAME" != "vscode@$package_version" ]]; then
	echo "Release tag must match pkgs/vscode/package.json (vscode@$package_version)." >&2
	exit 1
fi

echo -e "🚧 Publishing VS Code extension v$package_version ($package_target)"

echo
echo '╔═══════════════════════════════════════════════════╗'
./scripts/package.sh
echo '╚═══════════════════════════════════════════════════╝'

echo -e "\n🌀 Publishing to Visual Studio Marketplace..."

if ! output=$(fnox exec -- \
	pnpm exec vsce publish \
	--packagePath "$package_path" \
	--skip-duplicate \
	2>&1); then

	echo '╭─ 🔴 Failed ──────────────────────────────────────╮'
	echo "$output"
	echo '╰───────────────────────────────────────────────────╯'
	exit 1
fi

echo "🟢 Published"

echo -e "\n🌀 Publishing to Open VSX Registry..."

if ! output=$(fnox exec -- \
	pnpm exec ovsx publish "$package_path" --skip-duplicate 2>&1); then

	echo '╭─ 🔴 Failed ──────────────────────────────────────╮'
	echo "$output"
	echo '╰───────────────────────────────────────────────────╯'
	exit 1
fi

echo "🟢 Published"

echo -e "\n🎉 Extension shipped!"
