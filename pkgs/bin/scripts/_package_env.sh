#!/usr/bin/env bash

# This script provides paths for compiler building and linking.

set -euo pipefail

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
	echo "🔴 This script is meant to be sourced, not executed!" >&2
	exit 1
fi

eval "$(mise activate bash --shims)"
eval "$(mise env -s bash)"

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_dir="$(cd "$root_dir/../.." && pwd)"

export compiler_dir="$repo_dir/subs/errs"
export package_dist_dir="$root_dir/dist"
