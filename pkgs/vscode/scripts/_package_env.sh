#!/usr/bin/env bash

# This script provides environment variables for extension building & publishing.

set -euo pipefail

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
	echo "🔴 This script is meant to be sourced, not executed!" >&2
	exit 1
fi

eval "$(mise activate bash --shims)"
eval "$(mise env -s bash)"

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_dir="$(cd "$root_dir/../.." && pwd)"
extension_dir="$repo_dir/subs/errs-vscode/extensions/typescript-language-features"
export module_dir="$extension_dir/src/errs"

package_dist_dir="$root_dir/dist/production"
package_version="$(jaq -r '.version' "$root_dir/package.json")"
package_name="$(jaq -r '.name' "$extension_dir/package.json")"
package_target="$(node -p "process.platform + '-' + process.arch")"

package_filename="$package_name-$package_version-$package_target.vsix"
export package_path="$package_dist_dir/$package_filename"
