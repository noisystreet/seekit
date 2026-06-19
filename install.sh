#!/usr/bin/env bash
#
# seekit — CLI web search tool
# =============================
# Install script:  curl -fsSL https://raw.githubusercontent.com/noisystreet/seekit/main/install.sh | sh
#
# Detects platform (Linux / macOS, x86_64 / aarch64),
# downloads the latest release binary from GitHub,
# and installs it to /usr/local/bin (or ~/.local/bin as fallback).

set -euo pipefail

REPO="noisystreet/seekit"
BINARY="seekit"
VERSION="${1:-latest}"

# ── Colors ──────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'
BOLD='\033[1m'; NC='\033[0m'

info()  { printf "${GREEN}%s${NC}\n" "$*"; }
warn()  { printf "${RED}%s${NC}\n" "$*"; }
step()  { printf "${CYAN}==>${NC} ${BOLD}%s${NC}\n" "$*"; }

# ── Platform detection ───────────────────────────────────────────
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      warn "Unsupported OS: $(uname -s)"; exit 1 ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)      warn "Unsupported architecture: $(uname -m)"; exit 1 ;;
    esac

    echo "${arch}-${os}"
}

# ── Resolve version ─────────────────────────────────────────────
resolve_version() {
    if [ "$VERSION" != "latest" ]; then
        echo "$VERSION"
        return
    fi

    step "Fetching latest release version..."
    local url="https://api.github.com/repos/${REPO}/releases/latest"
    # Prefer GitHub token if available
    local auth=""
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        auth="Authorization: Bearer ${GITHUB_TOKEN}"
    fi

    local tag
    if [ -n "$auth" ]; then
        tag=$(curl -fsSL -H "$auth" "$url" | sed -n 's/.*"tag_name": "\(.*\)",/\1/p')
    else
        tag=$(curl -fsSL "$url" | sed -n 's/.*"tag_name": "\(.*\)",/\1/p')
    fi

    if [ -z "$tag" ]; then
        warn "Failed to fetch latest release. Falling back to 'latest'."
        echo "latest"
        return
    fi

    echo "$tag"
}

# ── Download & install ──────────────────────────────────────────
install_binary() {
    local platform="$1" version="$2"
    local archive="${BINARY}-${platform}.tar.gz"

    if [ "$version" = "latest" ]; then
        local url="https://github.com/${REPO}/releases/latest/download/${archive}"
    else
        local url="https://github.com/${REPO}/releases/download/${version}/${archive}"
    fi

    local tmpdir
    tmpdir=$(mktemp -d)
    cd "$tmpdir"

    step "Downloading ${BINARY} ${version} (${platform})..."
    curl -fsSL "$url" -o "$archive"

    step "Extracting..."
    tar xzf "$archive"
    chmod +x "$BINARY"

    # Choose install directory
    local dest=""
    if [ -w "/usr/local/bin" ]; then
        dest="/usr/local/bin"
    elif [ -w "${HOME}/.local/bin" ]; then
        dest="${HOME}/.local/bin"
    else
        mkdir -p "${HOME}/.local/bin"
        dest="${HOME}/.local/bin"
    fi

    # Check for existing binary
    if [ -f "${dest}/${BINARY}" ]; then
        local old_ver
        old_ver=$("${dest}/${BINARY}" --version 2>/dev/null || echo "unknown")
        warn "Overwriting existing installation: ${dest}/${BINARY} (version: ${old_ver})"
    fi

    mv "$BINARY" "${dest}/"
    rm -rf "$tmpdir"

    info "✓ Installed ${BINARY} to ${dest}/${BINARY}"

    # Check PATH
    if ! command -v "$BINARY" &>/dev/null; then
        warn "⚠ ${dest} is not in your PATH."
        warn "  Add it by running:  export PATH=\"${dest}:\$PATH\""
        warn "  Or add that line to your ~/.bashrc / ~/.zshrc"
    fi
}

# ── Verify ───────────────────────────────────────────────────────
verify() {
    if command -v "$BINARY" &>/dev/null; then
        info "✓ $(seekit --version 2>&1 || echo "${BINARY} installed")"
        info "  Try: seekit \"your search query\""
    fi
}

# ── Main ─────────────────────────────────────────────────────────
main() {
    printf "\n"
    step "Installing ${BINARY}..."
    printf "\n"

    local platform
    platform=$(detect_platform)

    local version
    version=$(resolve_version)

    install_binary "$platform" "$version"
    verify

    printf "\n"
    info "✓ Installation complete!"
    printf "\n"
}

main "$@"
