#!/usr/bin/env bash
# Builds the two versions for demo 3. Only the engine commit differs:
#   infino-demo:v1  engine before #1033: optimize loads manifest parts one after another
#   infino-demo:v2  engine at #1033: it loads them in parallel
set -euo pipefail
cd "$(dirname "$0")"

docker build --build-arg ENGINE_REV=589ed923f503aa987b94cfc4b06912667b7b6c41 -t infino-demo:v1 .
docker build --build-arg ENGINE_REV=9ec8f4054d4c42c4736d36d90fcb74f7a4430423 -t infino-demo:v2 .
