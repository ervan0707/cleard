#!/usr/bin/env bash

set -e

# GitHub repository information
REPO_OWNER="ervan0707"
REPO_NAME="cleard"
GITHUB_API="https://api.github.com"

error() { echo "Error: $1" >&2; exit 1; }
info()  { echo "$1"; }

# Detect OS and architecture
detect_platform() {
    case "$(uname -s)" in
        Linux*)     OS=linux;;
        Darwin*)    OS=darwin;;
        MINGW64*)   OS=windows;;
        MSYS*)      OS=windows;;
        *)          error "Unsupported operating system: $(uname -s)";;
    esac

    local arch
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64) ARCH=x86_64;;
        aarch64|arm64) ARCH=arm64;;
        *) error "Unsupported architecture: $arch";;
    esac

    if [ "$OS" = "windows" ]; then EXT=".zip"; else EXT=".tar.gz"; fi
}

get_latest_version() {
    info "Fetching latest release version..."
    LATEST_VERSION=$(curl -sL ${GITHUB_API}/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest | grep '"tag_name":' | cut -d'"' -f4)
    [ -z "$LATEST_VERSION" ] && error "Failed to fetch latest version"
    info "Latest version: $LATEST_VERSION"
}

install_binary() {
    local asset_name="cleard-${OS}-${ARCH}${EXT}"
    local download_url="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${LATEST_VERSION}/${asset_name}"
    local temp_dir; temp_dir=$(mktemp -d)
    local install_dir="$HOME/.local/bin"

    info "Downloading $asset_name..."
    curl -sL -o "${temp_dir}/${asset_name}" "$download_url" || error "Failed to download binary"
    mkdir -p "$install_dir" || error "Failed to create install directory"

    cd "$temp_dir"
    if [ "$OS" = "windows" ]; then
        unzip -q "${asset_name}" || error "Failed to extract binary"
        mv cleard.exe "$install_dir/" || error "Failed to install binary"
    else
        tar xzf "${asset_name}" || error "Failed to extract binary"
        if [ -f "cleard-${OS}-${ARCH}" ]; then
            mv "cleard-${OS}-${ARCH}" "$install_dir/cleard" || error "Failed to install binary"
        elif [ -f "cleard" ]; then
            mv "cleard" "$install_dir/cleard" || error "Failed to install binary"
        else
            error "Could not find binary after extraction."
        fi
        chmod +x "$install_dir/cleard" || error "Failed to set executable permissions"
    fi

    rm -rf "$temp_dir"
    info "Installed to: $install_dir/cleard"
    info "Make sure $install_dir is on your PATH."
}

main() {
    command -v curl >/dev/null 2>&1 || error "curl is required but not installed"
    command -v tar  >/dev/null 2>&1 || error "tar is required but not installed"
    info "Installing cleard..."
    detect_platform
    get_latest_version
    install_binary
    info "Installation complete!"
}

main
