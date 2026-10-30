#!/usr/bin/env bash
# Gets the infino engine (github.com/infino-ai/infino) at a commit, for the worker build.
#
#   ./get-engine.sh <full commit sha> <dir>
#
# One demo-only change: a manifest part holds 1 superfile instead of 10,000,
# so every append makes a new part and a small table has many parts (demo 3).
set -euo pipefail

rev="$1"
dir="$2"

git init -q "$dir"
cd "$dir"
git fetch -q --depth 1 https://github.com/infino-ai/infino.git "$rev"
git checkout -q FETCH_HEAD

file=src/supertable/options.rs
sed -i.bak 's/^const DEFAULT_TARGET_SUPERFILES_PER_PART: u64 = 10_000;/const DEFAULT_TARGET_SUPERFILES_PER_PART: u64 = 1;/' "$file"
rm "$file.bak"
grep -q '^const DEFAULT_TARGET_SUPERFILES_PER_PART: u64 = 1;' "$file"
