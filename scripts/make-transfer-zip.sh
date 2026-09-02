#!/bin/sh
set -eu

script_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_directory=$(dirname -- "$script_directory")
ready_directory="$project_directory/transfer-ready"
temporary_directory=$(mktemp -d)
temporary_zip="$temporary_directory/PulseBridge-Windows-Source.zip"
trap 'rm -rf "$temporary_directory"' EXIT INT TERM

mkdir -p "$ready_directory"
cd "$project_directory"
zip -qr "$temporary_zip" . \
  -x '.git/*' \
  -x 'node_modules/*' \
  -x '*/node_modules/*' \
  -x 'app/dist/*' \
  -x '*/dist/*' \
  -x '*.tsbuildinfo' \
  -x '*/*.tsbuildinfo' \
  -x 'src-tauri/target/*' \
  -x 'transfer-ready/*' \
  -x '.DS_Store' \
  -x '*.zip' \
  -x '*.log'
mv -f "$temporary_zip" "$ready_directory/PulseBridge-Windows-Source.zip"

echo "Created: $ready_directory/PulseBridge-Windows-Source.zip"
