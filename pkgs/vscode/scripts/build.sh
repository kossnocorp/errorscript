#!/usr/bin/env bash

# jaq expressions below intentionally use single quotes.
# shellcheck disable=SC2016

# This script builds the extension for Visual Studio Code.

set -euo pipefail

# shellcheck source-path=SCRIPTDIR
source "$(dirname "${BASH_SOURCE[0]}")/_package_env.sh"

cd "$root_dir"

dev=false
mode=production
bundle_args=(--minify)

if [[ "${1:-}" == "--dev" ]]; then
	dev=true
	mode=development
	bundle_args=(--sourcemap)
fi

DIST_DIR="$root_dir/dist/$mode"

echo -e "🚧 Building $mode Visual Studio Code extension..."

echo -e "\n🌀 Checking extension types..."

node "$module_dir/node_modules/typescript/bin/tsc" --noEmit -p "$module_dir/tsconfig.json"

echo "🟢 Types checked"

if [[ "$dev" == false ]]; then
	rm -rf "$DIST_DIR"
fi

mkdir -p "$DIST_DIR/server/libs" "$DIST_DIR/syntaxes"

echo -e "\n🌀 Building extension bundle..."

if ! output=$("$module_dir/node_modules/.bin/esbuild" "$module_dir/extension.ts" \
	--bundle --platform=node --format=cjs --target=node20 --external:vscode \
	--outfile="$DIST_DIR/extension.js" "${bundle_args[@]}" 2>&1); then

	echo '╭─ 🔴 `esbuild` failed ──────────────────────────────╮'
	echo "$output"
	echo '╰───────────────────────────────────────────────────╯'
	exit 1
fi

echo "🟢 Bundle built"

echo -e "\n🌀 Copying compiler and assets..."

ext=""

if [[ "$package_target" == win32-* ]]; then
	ext=".exe"
fi

# Replace atomically so an existing development host can keep running.
staged_path="$DIST_DIR/server/tsrs$ext.$$.tmp"
trap 'rm -f "$staged_path"' EXIT

cp "$repo_dir/pkgs/bin/dist/$mode/errs$ext" "$staged_path"
chmod +x "$staged_path"
mv -f "$staged_path" "$DIST_DIR/server/tsrs$ext"

cp "$repo_dir/subs/errs/crates/tsrs_vfs/libs/"lib*.d.ts "$DIST_DIR/server/libs/"
cp "$module_dir/error-options.schema.json" "$DIST_DIR/"
cp "$repo_dir/subs/errs-vscode/LICENSE.txt" "$DIST_DIR/"
cp "$root_dir/README.md" "$DIST_DIR/"

# This also works with a sparse VS Code checkout.
git -C "$extension_dir" show HEAD:extensions/typescript-basics/language-configuration.json \
	>"$DIST_DIR/language-configuration.json"

scopes=(source.ts source.tsx source.js source.js.jsx)

while IFS=$'\t' read -r index language; do
	jaq -n --arg scope "source.$language" --arg include "${scopes[$index]}" \
		'{scopeName: $scope, patterns: [{include: $include}]}' \
		>"$DIST_DIR/syntaxes/$language.json"
done < <(jaq -r '.languages | to_entries[] | [.key, .value.id] | @tsv' "$module_dir/contributes.json")

echo "🟢 Assets copied"

echo -e "\n🌀 Patching package.json..."

# Build the isolated ErrorScript manifest from the upstream metadata and contributions.
jaq --arg version "$package_version" --argjson dev "$dev" \
	--slurpfile contributes "$module_dir/contributes.json" '
  {
    name, publisher, displayName, description, license,
    version: $version,
    repository: {type: "git", url: "https://github.com/kossnocorp/errorscript.git"},
    homepage: "https://errorscript.org",
    engines: {vscode: "^1.100.0"},
    categories: ["Programming Languages"],
    main: "./extension.js",
    files: (["extension.js", "language-configuration.json", "error-options.schema.json",
      "server/**", "syntaxes/**", "LICENSE.txt", "README.md"] + if $dev then ["extension.js.map"] else [] end),
    activationEvents: [$contributes[0].languages[] | "onLanguage:" + .id],
    capabilities: {untrustedWorkspaces: {supported: false}, virtualWorkspaces: false},
    contributes: $contributes[0]
  }
' "$extension_dir/package.json" >"$DIST_DIR/package.json"

echo "🟢 package.json patched"

echo -e "\n🎉 Extension build is ready: $DIST_DIR"
