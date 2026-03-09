#!/usr/bin/env bash
set -euo pipefail

REPO="${NUVIX_CLI_REPO:-nuvix-dev/cli}"
VERSION="${NUVIX_VERSION:-latest}"
INSTALL_DIR="${NUVIX_INSTALL_DIR:-}"
BIN_NAME="${NUVIX_BIN_NAME:-nuvix}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd tar
need_cmd mktemp
need_cmd uname

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux) OS_KEY="linux" ;;
  Darwin) OS_KEY="darwin" ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH_KEY="x86_64" ;;
  aarch64|arm64) ARCH_KEY="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

TARGET="${ARCH_KEY}-unknown-${OS_KEY}-musl"
if [[ "$OS_KEY" == "darwin" ]]; then
  TARGET="${ARCH_KEY}-apple-darwin"
fi

FILE="nuvix-${TARGET}.tar.gz"
if [[ "$VERSION" == "latest" ]]; then
  URL="https://github.com/${REPO}/releases/latest/download/${FILE}"
else
  URL="https://github.com/${REPO}/releases/download/${VERSION}/${FILE}"
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

ARCHIVE_PATH="${TMP_DIR}/${FILE}"
EXTRACT_DIR="${TMP_DIR}/extract"
mkdir -p "$EXTRACT_DIR"

echo "Downloading ${URL}"
curl -fL --retry 3 --retry-delay 1 -o "$ARCHIVE_PATH" "$URL"
tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR"

SRC_BIN=""
if [[ -f "${EXTRACT_DIR}/nuvix" ]]; then
  SRC_BIN="${EXTRACT_DIR}/nuvix"
elif [[ -f "${EXTRACT_DIR}/cli" ]]; then
  SRC_BIN="${EXTRACT_DIR}/cli"
else
  echo "No supported binary found in archive (expected nuvix or cli)." >&2
  exit 1
fi

if [[ -z "$INSTALL_DIR" ]]; then
  if [[ -w "/usr/local/bin" ]]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
  fi
fi

mkdir -p "$INSTALL_DIR"
DEST_BIN="${INSTALL_DIR}/${BIN_NAME}"

if [[ -w "$INSTALL_DIR" ]]; then
  install -m 0755 "$SRC_BIN" "$DEST_BIN"
else
  if ! command -v sudo >/dev/null 2>&1; then
    echo "No write permission for ${INSTALL_DIR} and sudo is not available." >&2
    exit 1
  fi
  sudo install -m 0755 "$SRC_BIN" "$DEST_BIN"
fi

echo "Installed ${BIN_NAME} to ${DEST_BIN}"
echo "Run: ${BIN_NAME} --help"
