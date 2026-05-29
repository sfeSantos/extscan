#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-extscan}"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_FILE="${SCRIPT_DIR}/${APP_NAME}"
SOURCE_FILE="${SCRIPT_DIR}/main.rs"

mkdir -p "$INSTALL_DIR"

if [ -f "$BINARY_FILE" ]; then
    cp "$BINARY_FILE" "${INSTALL_DIR}/${APP_NAME}"
elif [ -f "$SOURCE_FILE" ]; then
    if ! command -v rustc >/dev/null 2>&1; then
        echo "Error: rustc was not found in PATH and no prebuilt ${APP_NAME} binary was found."
        echo "Download a release package with a prebuilt binary or install Rust first."
        exit 1
    fi

    rustc "$SOURCE_FILE" -o "${INSTALL_DIR}/${APP_NAME}"
else
    echo "Error: neither ${APP_NAME} nor main.rs was found."
    echo "Run this script from the project directory or from an extracted release package."
    exit 1
fi

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
