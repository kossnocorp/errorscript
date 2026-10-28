#!/usr/bin/env bash

# This script builds and stages the ErrorScript compiler.

set -euo pipefail

# shellcheck source-path=SCRIPTDIR
source "$(dirname "${BASH_SOURCE[0]}")/_package_env.sh"

target="${usage_target:-}"
dev="${usage_dev:-false}"

for arg in "$@"; do
	case "$arg" in
	--dev) dev=true ;;
	*) target="$arg" ;;
	esac
done

profile=release
mode=production
build_args=(--locked --package tsrs_cli --bin tsrs)

if [[ "$dev" == true ]]; then
	profile=debug
	mode=development
else
	build_args+=(--release)
fi

if [[ -n "$target" ]]; then
	build_args+=(--target "$target")
fi

echo -e "🚧 Building $mode ErrorScript compiler\n"

cd "$compiler_dir"
time cargo build "${build_args[@]}"

target_dir="$(cargo metadata --format-version 1 --no-deps | jaq -r '.target_directory')"
target_dir="${target_dir//\\//}"

host="${target:-$(rustc -vV | sed -n 's/^host: //p')}"
ext=""

if [[ "$host" == *windows* ]]; then
	ext=".exe"
fi

source_path="$target_dir/${target:+$target/}$profile/tsrs$ext"
dist_dir="$package_dist_dir/${target:+$target/}$mode"

mkdir -p "$dist_dir"

# Replace atomically so a running development host can keep using the previous binary.
staged_path="$dist_dir/errs$ext.$$.tmp"
trap 'rm -f "$staged_path"' EXIT

cp "$source_path" "$staged_path"
chmod +x "$staged_path"
mv -f "$staged_path" "$dist_dir/errs$ext"

echo -e "\n🎉 Compiler ready: $dist_dir/errs$ext"
