#!/usr/bin/env bash
# Builds the two versions for demo 3:
#   infino-demo:v1  the current code: the worker reads segments one after another
#   infino-demo:v2  with demo3-parallel-reads.patch: it reads them in parallel
set -euo pipefail
cd "$(dirname "$0")"

docker build -t infino-demo:v1 .

patch -p1 < demo3-parallel-reads.patch
trap 'patch -R -p1 < demo3-parallel-reads.patch' EXIT
docker build -t infino-demo:v2 .
