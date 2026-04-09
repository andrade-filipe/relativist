#!/bin/sh
# Relativist installer — downloads the latest release binary from GitHub.
# Usage: curl -sSfL https://raw.githubusercontent.com/andrade-filipe/relativist/main/scripts/install.sh | sh
#
# Environment variables:
#   INSTALL_DIR   — where to place the binary (default: /usr/local/bin, fallback: ~/.local/bin)
#   VERSION       — specific version to install (default: latest)

set -eu

REPO="andrade-filipe/relativist"
GITHUB_API="https://api.github.com/repos/${REPO}/releases"

# --- Detect OS and architecture ---

detect_os() {
  case "$(uname -s)" in
    Linux*)  echo "linux" ;;
    Darwin*) echo "darwin" ;;
    MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
    *)
      echo "Error: unsupported OS '$(uname -s)'." >&2
      echo "Supported: Linux, macOS. For Windows, download from GitHub Releases." >&2
      exit 1
      ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64) echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *)
      echo "Error: unsupported architecture '$(uname -m)'." >&2
      echo "Supported: x86_64, aarch64." >&2
      exit 1
      ;;
  esac
}

OS=$(detect_os)
ARCH=$(detect_arch)

# --- Map to target triple ---

case "${OS}-${ARCH}" in
  linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ; EXT="tar.gz" ;;
  linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ; EXT="tar.gz" ;;
  darwin-x86_64)  TARGET="x86_64-apple-darwin"       ; EXT="tar.gz" ;;
  darwin-aarch64) TARGET="aarch64-apple-darwin"       ; EXT="tar.gz" ;;
  *)
    echo "Error: no precompiled binary for ${OS}-${ARCH}." >&2
    echo "Install from source: cargo install --git https://github.com/${REPO}" >&2
    exit 1
    ;;
esac

# --- Resolve version ---

if [ -n "${VERSION:-}" ]; then
  TAG="v${VERSION#v}"
  RELEASE_URL="${GITHUB_API}/tags/${TAG}"
else
  RELEASE_URL="${GITHUB_API}/latest"
fi

echo "Detecting: OS=${OS} ARCH=${ARCH} TARGET=${TARGET}"
echo "Fetching release info from GitHub..."

RELEASE_JSON=$(curl -sSfL "${RELEASE_URL}") || {
  echo "Error: failed to fetch release info. Check your internet connection." >&2
  exit 1
}

TAG=$(echo "${RELEASE_JSON}" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
if [ -z "${TAG}" ]; then
  echo "Error: could not determine release tag." >&2
  exit 1
fi

echo "Installing relativist ${TAG}..."

# --- Download artifact and checksums ---

ARCHIVE_NAME="relativist-${TAG}-${TARGET}.${EXT}"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE_NAME}"
CHECKSUMS_URL="https://github.com/${REPO}/releases/download/${TAG}/SHA256SUMS"

TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR}"' EXIT

echo "Downloading ${ARCHIVE_NAME}..."
curl -sSfL -o "${TMPDIR}/${ARCHIVE_NAME}" "${DOWNLOAD_URL}" || {
  echo "Error: failed to download ${ARCHIVE_NAME}." >&2
  echo "This platform (${TARGET}) may not have a precompiled binary for ${TAG}." >&2
  echo "Install from source: cargo install --git https://github.com/${REPO} --tag ${TAG}" >&2
  exit 1
}

echo "Downloading checksums..."
curl -sSfL -o "${TMPDIR}/SHA256SUMS" "${CHECKSUMS_URL}" || {
  echo "Warning: SHA256SUMS not found, skipping checksum verification." >&2
}

# --- Verify checksum ---

if [ -f "${TMPDIR}/SHA256SUMS" ]; then
  echo "Verifying checksum..."
  cd "${TMPDIR}"
  if command -v sha256sum > /dev/null 2>&1; then
    grep "${ARCHIVE_NAME}" SHA256SUMS | sha256sum -c - || {
      echo "Error: checksum verification failed! The download may be corrupted." >&2
      exit 1
    }
  elif command -v shasum > /dev/null 2>&1; then
    grep "${ARCHIVE_NAME}" SHA256SUMS | shasum -a 256 -c - || {
      echo "Error: checksum verification failed! The download may be corrupted." >&2
      exit 1
    }
  else
    echo "Warning: neither sha256sum nor shasum found, skipping verification." >&2
  fi
  cd - > /dev/null
fi

# --- Extract binary ---

echo "Extracting..."
case "${EXT}" in
  tar.gz) tar xzf "${TMPDIR}/${ARCHIVE_NAME}" -C "${TMPDIR}" ;;
  zip)    unzip -q "${TMPDIR}/${ARCHIVE_NAME}" -d "${TMPDIR}" ;;
esac

BINARY_NAME="relativist"

# --- Install ---

INSTALL_DIR="${INSTALL_DIR:-}"

if [ -z "${INSTALL_DIR}" ]; then
  if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "${INSTALL_DIR}"
  fi
fi

cp "${TMPDIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

echo ""
echo "relativist ${TAG} installed to ${INSTALL_DIR}/${BINARY_NAME}"

# Check if INSTALL_DIR is in PATH
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "NOTE: ${INSTALL_DIR} is not in your PATH."
    echo "Add it with:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo ""
    echo "Or add that line to your ~/.bashrc or ~/.profile."
    ;;
esac

echo ""
echo "Verify: ${INSTALL_DIR}/${BINARY_NAME} --version"
