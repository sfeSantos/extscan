#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-extscan}"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"
SOURCE_FILE="main.rs"

if ! command -v rustc >/dev/null 2>&1; then
    echo "Error: rustc was not found in PATH."
    echo "Install Rust first: https://www.rust-lang.org/tools/install"
    exit 1
fi

if [ ! -f "$SOURCE_FILE" ]; then
    echo "Error: $SOURCE_FILE was not found."
    echo "Run this script from the project directory."
    exit 1
fi

mkdir -p "$INSTALL_DIR"
rustc "$SOURCE_FILE" -o "${INSTALL_DIR}/${APP_NAME}"
chmod +x "${INSTALL_DIR}/${APP_NAME}"

echo "Installed ${APP_NAME} at ${INSTALL_DIR}/${APP_NAME}"

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
        echo "You can now run: ${APP_NAME}"
        ;;
    *)
        echo
        echo "Warning: ${INSTALL_DIR} is not in your PATH."
        echo "Add this line to your shell configuration file:"
        echo "export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
