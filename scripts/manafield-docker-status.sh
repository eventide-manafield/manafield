#!/usr/bin/env bash
set -euo pipefail

PROJECT="manafield"

docker ps   --filter "label=com.docker.compose.project=$PROJECT"   --format 'table {{.Names}}	{{.Image}}	{{.Status}}	{{.Ports}}'
