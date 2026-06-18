#!/usr/bin/env bash
# Installer for codexswitch-cli
# Detects OS/arch, downloads binary from releases, verifies checksum from release

set -euo pipefail

VERSION="${CODEXSWITCH_CLI_VERSION:-}"
REPO="syntaxskills/codexswitch-cli"
INSTALL_DIR="${CODEXSWITCH_CLI_INSTALL_DIR:-$HOME/.local/bin}"
ALLOW_INSECURE_INSTALL="${CODEXSWITCH_CLI_ALLOW_INSECURE_INSTALL:-0}"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    BOLD='\033[1m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    RESET='\033[0m'
else
    BOLD='' GREEN='' YELLOW='' RED='' RESET=''
fi

info() {
    printf "${GREEN}==>${RESET} ${BOLD}%s${RESET}\n" "$*" >&2
}

warn() {
    printf "${YELLOW}warning:${RESET} %s\n" "$*" >&2
}

error() {
    printf "${RED}error:${RESET} %b\n" "$*" >&2
    exit 1
}

need_cmd() {
    if ! command -v "$1" > /dev/null 2>&1; then
        error "need '$1' (command not found)"
    fi
}

normalize_version() {
    printf '%s' "${1#v}"
}

is_valid_version() {
    [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]]
}

fetch_text() {
    local url="$1"

    if command -v curl > /dev/null 2>&1; then
        curl -fsSL --proto '=https' --tlsv1.2 "$url" || return 1
    elif command -v wget > /dev/null 2>&1; then
        wget -qO- --https-only --secure-protocol=TLSv1_2 "$url" || return 1
    else
        return 1
    fi
}

extract_json_string() {
    local key="$1"
    local payload="$2"

    printf '%s' "$payload" \
        | tr -d '\r\n' \
        | sed -n "s/.*\"${key}\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" \
        | head -n 1
}

fetch_latest_version_from_github() {
    local payload version

    payload="$(fetch_text "https://api.github.com/repos/${REPO}/releases/latest")" || return 1
    version="$(extract_json_string "tag_name" "$payload")"
    version="$(normalize_version "$version")"
    is_valid_version "$version" || return 1
    printf '%s' "$version"
}

resolve_version() {
    local latest

    if [ -n "${VERSION:-}" ]; then
        VERSION="$(normalize_version "$VERSION")"
        is_valid_version "$VERSION" || error "invalid version '${VERSION}'"
        return
    fi

    info "Resolving latest released version"

    latest="$(fetch_latest_version_from_github)" \
        || error "could not resolve the latest published GitHub release automatically.\nSet CODEXSWITCH_CLI_VERSION or pass --version to install a specific release."

    VERSION="$latest"
    info "Resolved version from GitHub releases: v${VERSION}"
}

version_help_text() {
    if [ -n "${VERSION:-}" ]; then
        printf 'v%s' "$VERSION"
    else
        printf 'latest release'
    fi
}

detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)     os="linux" ;;
        Darwin*)    os="darwin" ;;
        MINGW*|MSYS*|CYGWIN*|Windows_NT) os="windows" ;;
        *)          error "unsupported OS: $(uname -s)" ;;
    esac

    local machine
    machine="$(uname -m)"
    case "$machine" in
        x86_64|amd64)       arch="x86_64" ;;
        aarch64|arm64)      arch="aarch64" ;;
        *)                  error "unsupported architecture: $machine" ;;
    esac

    case "$os-$arch" in
        linux-x86_64)       echo "x86_64-unknown-linux-gnu" ;;
        linux-aarch64)      echo "aarch64-unknown-linux-gnu" ;;
        darwin-x86_64)      echo "x86_64-apple-darwin" ;;
        darwin-aarch64)     echo "aarch64-apple-darwin" ;;
        windows-x86_64)     echo "x86_64-pc-windows-msvc" ;;
        *)                  error "unsupported platform: $os-$arch" ;;
    esac
}

download_file() {
    local url="$1"
    local output="$2"
    local show_progress="${3:-false}"
    
    if command -v curl > /dev/null 2>&1; then
        if [ "$show_progress" = "true" ] && [ -t 1 ]; then
            # Show progress bar if stdout is a TTY
            curl -#fL --proto '=https' --tlsv1.2 "$url" -o "$output" || return 1
        else
            curl -fsSL --proto '=https' --tlsv1.2 "$url" -o "$output" || return 1
        fi
    elif command -v wget > /dev/null 2>&1; then
        if [ "$show_progress" = "true" ] && [ -t 1 ]; then
            # Show progress bar if stdout is a TTY
            wget --https-only --secure-protocol=TLSv1_2 --show-progress "$url" -O "$output" || return 1
        else
            wget -q --https-only --secure-protocol=TLSv1_2 "$url" -O "$output" || return 1
        fi
    else
        error "need 'curl' or 'wget' to download"
    fi
}

verify_checksum() {
    local file="$1"
    local checksum_file="$2"
    
    local basename
    basename="$(basename "$file")"
    local expected actual
    
    expected="$(
        awk -v name="$basename" '
            $2 == name || $2 ~ ("(^|/)release/" name "$") { print $1; exit }
        ' "$checksum_file"
    )"
    if [ -z "$expected" ]; then
        error "checksum not found for $basename in checksum file"
    fi
    
    if command -v sha256sum > /dev/null 2>&1; then
        actual="$(sha256sum "$file" | awk '{print $1}')"
    elif command -v shasum > /dev/null 2>&1; then
        actual="$(shasum -a 256 "$file" | awk '{print $1}')"
    elif command -v openssl > /dev/null 2>&1; then
        actual="$(openssl dgst -sha256 "$file" | awk '{print $NF}')"
    else
        if [[ "${ALLOW_INSECURE_INSTALL}" == "1" ]]; then
            warn "sha256sum/shasum/openssl not found, skipping checksum verification because CODEXSWITCH_CLI_ALLOW_INSECURE_INSTALL=1"
            return 0
        fi
        error "need sha256sum, shasum, or openssl for checksum verification.\nSet CODEXSWITCH_CLI_ALLOW_INSECURE_INSTALL=1 to bypass (not recommended)."
    fi
    
    if [ "$expected" != "$actual" ]; then
        error "checksum mismatch!\n  expected: $expected\n  actual:   $actual"
    fi
    
    info "Checksum verified ✓"
}

cleanup() {
    if [ -n "${TMPDIR_INSTALL:-}" ] && [ -d "$TMPDIR_INSTALL" ]; then
        rm -rf "$TMPDIR_INSTALL"
    fi
}

main() {
    need_cmd uname
    need_cmd mkdir
    need_cmd chmod
    resolve_version
    
    info "Installing codexswitch-cli v$VERSION"
    
    local target
    target="$(detect_platform)"
    info "Detected platform: $target"
    
    local base_url="https://github.com/$REPO/releases/download/v$VERSION"
    local archive_name="codexswitch-cli-${target}.tar.gz"
    local is_windows=0
    if [[ "$target" == *"windows"* ]]; then
        archive_name="codexswitch-cli-${target}.exe.zip"
        is_windows=1
        need_cmd unzip
    else
        need_cmd tar
    fi
    local archive_url="$base_url/$archive_name"
    
    local checksum_url="$base_url/SHA256SUMS"
    
    TMPDIR_INSTALL="$(mktemp -d)"
    trap cleanup EXIT
    local tmpdir="$TMPDIR_INSTALL"
    
    local archive_path="$tmpdir/$archive_name"
    local checksum_path="$tmpdir/checksums.txt"
    
    info "Downloading binary..."
    download_file "$archive_url" "$archive_path" "true" || error "failed to download binary from $archive_url"
    
    info "Downloading checksums from release..."
    if ! download_file "$checksum_url" "$checksum_path" "false"; then
        if [[ "${ALLOW_INSECURE_INSTALL}" == "1" ]]; then
            warn "Could not download checksum file from release"
            warn "Proceeding without verification because CODEXSWITCH_CLI_ALLOW_INSECURE_INSTALL=1"
        else
            error "could not download checksum file from release; aborting install.\nSet CODEXSWITCH_CLI_ALLOW_INSECURE_INSTALL=1 to bypass (not recommended)."
        fi
    else
        verify_checksum "$archive_path" "$checksum_path"
    fi
    
    info "Extracting..."
    if [[ "${is_windows}" -eq 1 ]]; then
        unzip -q "$archive_path" -d "$tmpdir" || error "extraction failed"
    else
        tar -xzf "$archive_path" -C "$tmpdir" || error "extraction failed"
    fi
    
    # Determine binary name based on OS
    local binary_name="codexswitch-cli"
    if [[ "$target" == *"windows"* ]]; then
        binary_name="codexswitch-cli.exe"
    fi
    
    local binary_path
    if [ -f "$tmpdir/$binary_name" ]; then
        binary_path="$tmpdir/$binary_name"
    elif [ -f "$tmpdir/codexswitch-cli/$binary_name" ]; then
        binary_path="$tmpdir/codexswitch-cli/$binary_name"
    else
        error "binary not found in archive (looking for $binary_name)"
    fi
    
    info "Installing to $INSTALL_DIR/$binary_name"
    mkdir -p "$INSTALL_DIR"
    
    if [ -f "$INSTALL_DIR/$binary_name" ]; then
        local backup
        backup="$INSTALL_DIR/$binary_name.backup.$(date +%s)"
        mv "$INSTALL_DIR/$binary_name" "$backup"
        info "Backed up existing binary to $backup"
    fi
    
    cp "$binary_path" "$INSTALL_DIR/$binary_name"
    
    # Make executable on Unix-like systems (not needed on Windows)
    if [[ "$target" != *"windows"* ]]; then
        chmod +x "$INSTALL_DIR/$binary_name"
    fi
    
    if [ -f "$INSTALL_DIR/$binary_name" ]; then
        local installed_version
        if installed_version="$("$INSTALL_DIR/$binary_name" --version 2>&1)"; then
            installed_version="$(printf '%s\n' "$installed_version" | head -n 1)"
            if [ -n "$installed_version" ]; then
                info "Successfully installed: $installed_version"
            else
                warn "installed binary did not report a version"
                info "Successfully installed: $INSTALL_DIR/$binary_name"
            fi
        else
            error "installation failed: '$INSTALL_DIR/$binary_name --version' did not run successfully:\n$installed_version"
        fi
    else
        error "installation failed: binary was not copied"
    fi
    
    local install_dir_no_trailing="${INSTALL_DIR%/}"
    if [[ ":$PATH:" != *":${install_dir_no_trailing}:"* ]]; then
        warn "$INSTALL_DIR is not in your PATH"
        if [[ "$target" == *"windows"* ]]; then
            warn "Add this directory to your PATH environment variable"
            warn "Or run: setx PATH \"%PATH%;$INSTALL_DIR\""
        else
            warn "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
            echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        fi
    else
        info "Installation complete! Run: $binary_name --help"
    fi
}

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Install codexswitch-cli by downloading the correct binary for your platform.

Options:
  -v, --version VERSION    Install specific version (default: $(version_help_text))
  -d, --dir DIR            Install to directory (default: $INSTALL_DIR)
  -h, --help               Show this help message

Environment variables:
  CODEXSWITCH_CLI_VERSION          Override auto-detected release version
  CODEXSWITCH_CLI_INSTALL_DIR      Override default install directory
  CODEXSWITCH_CLI_ALLOW_INSECURE_INSTALL  Set to 1 to bypass checksum requirement
  NO_COLOR                        Disable colored output

Security:
  Checksums are downloaded from the tagged GitHub release and used to verify
  the downloaded binary before installation.

Examples:
  $0                              # Install the latest published release
  $0 --version 1.2.3             # Install a specific version
  $0 --dir /usr/local/bin        # Install to custom directory

EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -v|--version)
            if [[ $# -lt 2 ]]; then
                error "missing value for $1"
            fi
            VERSION="${2#v}"
            shift 2
            ;;
        -d|--dir)
            if [[ $# -lt 2 ]]; then
                error "missing value for $1"
            fi
            INSTALL_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            error "unknown option: $1\nRun '$0 --help' for usage."
            ;;
    esac
done

main
