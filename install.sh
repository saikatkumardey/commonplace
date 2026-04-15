#!/bin/sh
set -e

REPO="saikatkumardey/commonplace"
INSTALL_DIR="${COMMONPLACE_INSTALL_DIR:-/usr/local/bin}"

# Detect OS and arch
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux)  OS_NAME="linux" ;;
    darwin) OS_NAME="macos" ;;
    *)      echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64)  ARCH_NAME="amd64" ;;
    aarch64|arm64) ARCH_NAME="arm64" ;;
    *)             echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

BINARY="commonplace-${OS_NAME}-${ARCH_NAME}"
URL="https://github.com/${REPO}/releases/latest/download/${BINARY}.tar.gz"

echo "Installing commonplace (${OS_NAME}/${ARCH_NAME})..."

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

curl -fsSL "$URL" -o "$TMPDIR/commonplace.tar.gz"
tar xzf "$TMPDIR/commonplace.tar.gz" -C "$TMPDIR"

EXTRACTED=$(find "$TMPDIR" -maxdepth 1 -type f -name "commonplace*" | head -1)
if [ -z "$EXTRACTED" ]; then
    echo "Error: could not find commonplace binary in archive" >&2
    exit 1
fi

if [ -w "$INSTALL_DIR" ]; then
    mv "$EXTRACTED" "$INSTALL_DIR/commonplace"
else
    sudo mv "$EXTRACTED" "$INSTALL_DIR/commonplace"
fi

chmod +x "$INSTALL_DIR/commonplace"

echo "Installed commonplace to $INSTALL_DIR/commonplace"
commonplace --help
