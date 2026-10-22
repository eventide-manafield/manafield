#!/bin/sh
set -eu
# Same entry point from an operator shell or Jenkins agent. No deployment step.
exec python3 "$(dirname "$0")/release.py" "$@"
