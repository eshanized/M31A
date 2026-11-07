#!/usr/bin/env bash
# ==============================================================================
# M31A Installer for Linux
# High-Performance Autonomous Agentic Coding Assistant powered by NVIDIA NIM
# https://github.com/eshanized/M31A
# ==============================================================================

set -euo pipefail

APP="m31a"
REPO="eshanized/M31A"
DEFAULT_VERSION="0.1.14"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.m31a/bin}"

# ANSI color codes
MUTED='\033[0;2m'
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
ORANGE='\033[38;5;214m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Helper for colored logging
log_info() {
    echo -e "${MUTED}[*]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_warning() {
    echo -e "${ORANGE}[!] Warning:${NC} $1" >&2
}

log_error() {
    echo -e "${RED}[✗] Error:${NC} $1" >&2
}

# Print M31A banner
print_banner() {
    echo -e "${CYAN}"
    cat << 'EOF'
  ███╗   ███╗██████╗  ██╗ █████╗ 
  ████╗ ████║╚════██╗███║██╔══██╗
  ██╔████╔██║ █████╔╝╚██║███████║
  ██║╚██╔╝██║ ╚═══██╗ ██║██╔══██║
  ██║ ╚═╝ ██║██████╔╝ ██║██║  ██║
  ╚═╝     ╚═╝╚═════╝  ╚═╝╚═╝  ╚═╝
EOF
    echo -e "${NC}${MUTED}Autonomous Agentic Coding Assistant powered by NVIDIA NIM${NC}\n"
}

usage() {
    cat <<EOF
M31A Installer for Linux

Usage: install.sh [options]

Options:
    -h, --help              Display this help message
    -v, --version <version> Install a specific version (e.g. 0.1.14 or latest)
    -b, --binary <path>     Install from an existing local binary instead of downloading
    -d, --dir <path>        Custom installation directory (default: ~/.m31a/bin)
        --build             Build from local source using Cargo
        --no-modify-path    Do not modify shell configuration files (.bashrc, .zshrc, etc.)
        --system            Install system-wide to /usr/local/bin (requires root/sudo)
        --uninstall         Run the uninstaller to remove M31A and all local data

Examples:
    curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash
    ./install.sh --version 0.1.14
    ./install.sh --binary ./target/release/m31a
    ./install.sh --no-modify-path
    ./install.sh --uninstall
EOF
}

# If first arg is uninstall / --uninstall, forward immediately to uninstaller
if [ "${1:-}" = "--uninstall" ] || [ "${1:-}" = "uninstall" ]; then
    shift
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
    if [ -f "$script_dir/uninstall.sh" ]; then
        exec bash "$script_dir/uninstall.sh" "$@"
    elif [ -f "$script_dir/scripts/uninstall.sh" ]; then
        exec bash "$script_dir/scripts/uninstall.sh" "$@"
    elif [ -f "$HOME/.m31a/uninstall.sh" ]; then
        exec bash "$HOME/.m31a/uninstall.sh" "$@"
    else
        log_info "Fetching uninstaller utility..."
        curl -fsSL "https://raw.githubusercontent.com/$REPO/master/scripts/uninstall.sh" | bash -s -- "$@"
        exit $?
    fi
fi

# Parse command line flags
requested_version="${VERSION:-}"
no_modify_path="${NO_MODIFY_PATH:-false}"
binary_path="${M31A_BINARY:-}"
build_from_source="${BUILD_FROM_SOURCE:-false}"
run_uninstall=false
is_system=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            usage
            exit 0
            ;;
        -v|--version)
            if [[ -n "${2:-}" ]]; then
                requested_version="$2"
                shift 2
            else
                log_error "--version requires a version argument"
                exit 1
            fi
            ;;
        -b|--binary)
            if [[ -n "${2:-}" ]]; then
                binary_path="$2"
                shift 2
            else
                log_error "--binary requires a file path"
                exit 1
            fi
            ;;
        -d|--dir|--install-dir)
            if [[ -n "${2:-}" ]]; then
                INSTALL_DIR="$2"
                shift 2
            else
                log_error "--dir requires a directory path"
                exit 1
            fi
            ;;
        --build)
            build_from_source=true
            shift
            ;;
        --no-modify-path)
            no_modify_path=true
            shift
            ;;
        --system)
            is_system=true
            INSTALL_DIR="/usr/local/bin"
            shift
            ;;
        --uninstall)
            run_uninstall=true
            shift
            ;;
        *)
            log_warning "Unknown option '$1'"
            shift
            ;;
    esac
done

# If uninstall requested via options
if [ "$run_uninstall" = "true" ]; then
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
    if [ -f "$script_dir/uninstall.sh" ]; then
        exec bash "$script_dir/uninstall.sh" "$@"
    elif [ -f "$script_dir/scripts/uninstall.sh" ]; then
        exec bash "$script_dir/scripts/uninstall.sh" "$@"
    elif [ -f "$HOME/.m31a/uninstall.sh" ]; then
        exec bash "$HOME/.m31a/uninstall.sh" "$@"
    else
        log_info "Fetching uninstaller utility..."
        curl -fsSL "https://raw.githubusercontent.com/$REPO/master/scripts/uninstall.sh" | bash -s -- "$@"
        exit $?
    fi
fi

print_banner

# Check operating system
raw_os="$(uname -s)"
case "$raw_os" in
    Linux*)
        ;;
    Darwin*)
        log_warning "M31A is primary-tested on Linux (bwrap sandbox). Darwin/macOS support is experimental."
        ;;
    *)
        log_error "Unsupported operating system: $raw_os. M31A requires Linux."
        exit 1
        ;;
esac

# Check architecture
raw_arch="$(uname -m)"
arch=""
case "$raw_arch" in
    x86_64|amd64)
        arch="x64"
        ;;
    aarch64|arm64)
        arch="arm64"
        ;;
    *)
        log_error "Unsupported architecture: $raw_arch"
        exit 1
        ;;
esac

# Check permissions for system directory
if [ "$is_system" = "true" ] && [ "$EUID" -ne 0 ]; then
    log_error "System installation into /usr/local/bin requires root privileges. Please re-run with sudo."
    exit 1
fi

# Check required utilities
for cmd in curl tar; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        log_error "'$cmd' is required but not installed. Please install it with your package manager."
        exit 1
    fi
done

# Check optional sandbox dependency (Bubblewrap)
if ! command -v bwrap >/dev/null 2>&1; then
    echo -e "${MUTED}╭──────────────────────────────────────────────────────────────────────────╮${NC}"
    echo -e "${MUTED}│${NC} ${ORANGE}Notice:${NC} Bubblewrap (bwrap) is not detected on your system.            ${MUTED}│${NC}"
    echo -e "${MUTED}│${NC} M31A uses Bubblewrap for secure filesystem and process sandboxing.      ${MUTED}│${NC}"
    echo -e "${MUTED}│${NC} You can install it anytime using:                                       ${MUTED}│${NC}"
    echo -e "${MUTED}│${NC}   ${BOLD}Debian/Ubuntu:${NC}  sudo apt-get install -y bubblewrap                  ${MUTED}│${NC}"
    echo -e "${MUTED}│${NC}   ${BOLD}Fedora/RHEL:${NC}    sudo dnf install -y bubblewrap                      ${MUTED}│${NC}"
    echo -e "${MUTED}│${NC}   ${BOLD}Arch Linux:${NC}     sudo pacman -S --noconfirm bubblewrap               ${MUTED}│${NC}"
    echo -e "${MUTED}╰──────────────────────────────────────────────────────────────────────────╯${NC}\n"
fi

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

# Download progress bar rendering helper
download_with_progress() {
    local url="$1"
    local output="$2"

    if [ -t 2 ]; then
        exec 4>&2
    else
        exec 4>/dev/null
    fi

    local headers="${output}.headers"
    printf "\033[?25l" >&4 # hide cursor
    trap "trap - RETURN; rm -f \"$headers\" 2>/dev/null; printf '\033[?25h' >&4; exec 4>&-" RETURN

    curl -fsSL -D "$headers" -o "$output" "$url" &
    local curl_pid=$!

    local length bytes
    local width=40
    while kill -0 "$curl_pid" 2>/dev/null; do
        if [ -f "$output" ]; then
            length=$(grep -i '^content-length:' "$headers" 2>/dev/null | tail -n 1 | tr -dc '0-9' || true)
            bytes=$(wc -c < "$output" 2>/dev/null || echo 0)
            if [ -n "$length" ] && [ "$length" -gt 0 ]; then
                local percent=$(( bytes * 100 / length ))
                [ "$percent" -gt 100 ] && percent=100
                local on=$(( percent * width / 100 ))
                local off=$(( width - on ))
                local filled=$(printf "%*s" "$on" "")
                filled=${filled// /■}
                local empty=$(printf "%*s" "$off" "")
                empty=${empty// /･}
                printf "\r${CYAN}%s%s %3d%%${NC} (%s bytes)" "$filled" "$empty" "$percent" "$bytes" >&4
            fi
        fi
        sleep 0.1 2>/dev/null || sleep 1
    done

    local ret=0
    wait "$curl_pid" || ret=$?
    printf "\r\033[K" >&4 # Clear progress line
    return $ret
}

# Resolve version
resolve_version() {
    if [ -n "$requested_version" ] && [ "$requested_version" != "latest" ]; then
        requested_version="${requested_version#v}"
        echo "$requested_version"
        return
    fi

    log_info "Fetching latest release information from GitHub..."
    local latest_tag
    latest_tag=$(curl -fsSL -H "Accept: application/vnd.github.v3+json" "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name":' | head -n 1 | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/' || true)
    
    if [ -n "$latest_tag" ]; then
        latest_tag="${latest_tag#v}"
        echo "$latest_tag"
    else
        log_warning "Could not reach GitHub Releases API (rate-limit or offline). Falling back to $DEFAULT_VERSION."
        echo "$DEFAULT_VERSION"
    fi
}

# Verify checksum if sha256sum and hash file available
verify_checksum() {
    local file_path="$1"
    local checksums_file="$2"
    local filename
    filename="$(basename "$file_path")"

    if [ -f "$checksums_file" ] && command -v sha256sum >/dev/null 2>&1; then
        local expected_hash
        expected_hash=$(grep "$filename" "$checksums_file" | awk '{print $1}' || true)
        if [ -n "$expected_hash" ]; then
            local actual_hash
            actual_hash=$(sha256sum "$file_path" | awk '{print $1}')
            if [ "$actual_hash" = "$expected_hash" ]; then
                log_success "Verified SHA-256 checksum for $filename"
            else
                log_error "SHA-256 checksum mismatch for $filename!"
                log_error "Expected: $expected_hash"
                log_error "Actual:   $actual_hash"
                exit 1
            fi
        fi
    fi
}

# Build from source using Cargo
build_source() {
    log_info "Building M31A from local source using Cargo..."
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "Cargo is not installed. Please install Rust and Cargo via https://rustup.rs"
        exit 1
    fi

    cargo build --release -p m31a-cli
    local built_bin="target/release/$APP"
    if [ ! -f "$built_bin" ]; then
        log_error "Failed to locate compiled binary at $built_bin"
        exit 1
    fi

    cp "$built_bin" "$INSTALL_DIR/$APP"
    chmod 755 "$INSTALL_DIR/$APP"
    log_success "Built and installed M31A binary to $INSTALL_DIR/$APP"
}

# Install from provided local binary
install_from_binary() {
    log_info "Installing M31A from local binary: $binary_path"
    if [ ! -f "$binary_path" ]; then
        log_error "Binary not found at: $binary_path"
        exit 1
    fi

    cp "$binary_path" "$INSTALL_DIR/$APP"
    chmod 755 "$INSTALL_DIR/$APP"
    log_success "Installed binary to $INSTALL_DIR/$APP"
}

# Download and install release asset
download_and_install() {
    local version
    version=$(resolve_version)
    log_info "Selected M31A version: ${BOLD}v${version}${NC}"

    local tmp_dir
    tmp_dir=$(mktemp -d "/tmp/m31a_install_XXXXXX")
    trap 'rm -rf "$tmp_dir"' EXIT

    local base_url="https://github.com/$REPO/releases/download/v${version}"
    local tarball_name="m31a-${version}-linux-${arch}.tar.gz"
    local raw_bin_name="m31a-${version}-linux-${raw_arch}"
    local download_success=false

    # Attempt to download SHA256SUMS for integrity validation
    local checksums_file="$tmp_dir/SHA256SUMS"
    curl -fsSL "${base_url}/SHA256SUMS" -o "$checksums_file" 2>/dev/null || true

    # Try tarball first
    local tarball_url="${base_url}/${tarball_name}"
    log_info "Downloading ${BOLD}${tarball_name}${NC}..."
    if download_with_progress "$tarball_url" "$tmp_dir/$tarball_name"; then
        verify_checksum "$tmp_dir/$tarball_name" "$checksums_file"

        log_info "Extracting archive..."
        tar -xzf "$tmp_dir/$tarball_name" -C "$tmp_dir"
        
        # Locate binary within extracted archive
        local extracted_bin
        extracted_bin=$(find "$tmp_dir" -type f -name "$APP" 2>/dev/null | head -n 1 || true)
        if [ -n "$extracted_bin" ] && [ -f "$extracted_bin" ]; then
            mv "$extracted_bin" "$INSTALL_DIR/$APP"
            chmod 755 "$INSTALL_DIR/$APP"
            download_success=true
        fi
    fi

    # Fallback to standalone binary if tarball was not found or extraction failed
    if [ "$download_success" != "true" ]; then
        local raw_url="${base_url}/${raw_bin_name}"
        log_info "Tarball not found. Attempting standalone binary: ${BOLD}${raw_bin_name}${NC}..."
        if download_with_progress "$raw_url" "$tmp_dir/$APP"; then
            verify_checksum "$tmp_dir/$APP" "$checksums_file"
            mv "$tmp_dir/$APP" "$INSTALL_DIR/$APP"
            chmod 755 "$INSTALL_DIR/$APP"
            download_success=true
        fi
    fi

    if [ "$download_success" != "true" ]; then
        log_error "Failed to download M31A release asset for linux-${arch} from GitHub."
        echo -e "${MUTED}Release URL: ${base_url}${NC}"
        echo -e "${MUTED}You can build from source with: ${NC}./install.sh --build"
        echo -e "${MUTED}Or provide a pre-compiled binary with: ${NC}./install.sh --binary /path/to/m31a"
        exit 1
    fi

    log_success "Successfully installed M31A binary to $INSTALL_DIR/$APP"
}

# Perform installation
if [ -n "$binary_path" ]; then
    install_from_binary
elif [ "$build_from_source" = "true" ]; then
    build_source
else
    download_and_install
fi

# Verify executable
if [ -x "$INSTALL_DIR/$APP" ]; then
    installed_ver=$("$INSTALL_DIR/$APP" --version 2>/dev/null || echo "v$DEFAULT_VERSION")
    log_success "M31A binary verified: ${BOLD}$installed_ver${NC}"
else
    log_error "Installed binary is missing or not executable at $INSTALL_DIR/$APP"
    exit 1
fi

# Save uninstaller copy for convenience
ensure_uninstaller() {
    local user_m31a_dir="$HOME/.m31a"
    mkdir -p "$user_m31a_dir"
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
    
    if [ -f "$script_dir/uninstall.sh" ]; then
        cp "$script_dir/uninstall.sh" "$user_m31a_dir/uninstall.sh"
    elif [ -f "$script_dir/scripts/uninstall.sh" ]; then
        cp "$script_dir/scripts/uninstall.sh" "$user_m31a_dir/uninstall.sh"
    else
        log_info "Downloading uninstaller utility..."
        curl -fsSL "https://raw.githubusercontent.com/$REPO/master/scripts/uninstall.sh" -o "$user_m31a_dir/uninstall.sh" 2>/dev/null || true
    fi

    if [ -f "$user_m31a_dir/uninstall.sh" ]; then
        chmod 755 "$user_m31a_dir/uninstall.sh"
        # Create convenient CLI uninstaller command in INSTALL_DIR
        mkdir -p "$INSTALL_DIR"
        cat > "$INSTALL_DIR/m31a-uninstall" <<'EOF'
#!/usr/bin/env bash
exec "$HOME/.m31a/uninstall.sh" "$@"
EOF
        chmod 755 "$INSTALL_DIR/m31a-uninstall"
    fi
}
ensure_uninstaller

# Configure PATH in shell config files
add_path_to_config() {
    local rc_file="$1"
    local is_fish="${2:-false}"

    if [ ! -f "$rc_file" ]; then
        touch "$rc_file" 2>/dev/null || return 0
    fi

    if grep -q "m31a path initialization" "$rc_file" 2>/dev/null; then
        log_info "PATH configuration already present in $rc_file"
        return 0
    fi

    if [ "$is_fish" = "true" ]; then
        cat >> "$rc_file" <<EOF

# >>> m31a path initialization >>>
fish_add_path "$INSTALL_DIR"
# <<< m31a path initialization <<<
EOF
    else
        cat >> "$rc_file" <<EOF

# >>> m31a path initialization >>>
export PATH="$INSTALL_DIR:\$PATH"
# <<< m31a path initialization <<<
EOF
    fi
    log_success "Added $INSTALL_DIR to PATH in $rc_file"
}

configure_path() {
    if [ "$no_modify_path" = "true" ]; then
        log_info "Skipping PATH modifications (--no-modify-path was passed)"
        return 0
    fi

    # Check if already in PATH
    if [[ ":$PATH:" == *":$INSTALL_DIR:"* ]]; then
        log_info "$INSTALL_DIR is already in your \$PATH"
        return 0
    fi

    local current_shell
    current_shell=$(basename "${SHELL:-bash}")
    local configured=false

    case "$current_shell" in
        fish)
            if [ -d "$HOME/.config/fish" ]; then
                add_path_to_config "$HOME/.config/fish/config.fish" true
                configured=true
            fi
            ;;
        zsh)
            for file in "${ZDOTDIR:-$HOME}/.zshrc" "$HOME/.zprofile"; do
                if [ -f "$file" ] || [ "$file" = "${ZDOTDIR:-$HOME}/.zshrc" ]; then
                    add_path_to_config "$file" false
                    configured=true
                    break
                fi
            done
            ;;
        bash|*)
            for file in "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.profile"; do
                if [ -f "$file" ]; then
                    add_path_to_config "$file" false
                    configured=true
                    break
                fi
            done
            ;;
    esac

    if [ "$configured" != "true" ]; then
        log_warning "Could not detect active shell rc file. Please add manually:"
        echo -e "    ${CYAN}export PATH=\"$INSTALL_DIR:\$PATH\"${NC}"
    fi

    # Support GitHub Actions environment
    if [ -n "${GITHUB_ACTIONS:-}" ] && [ "$GITHUB_ACTIONS" = "true" ] && [ -n "${GITHUB_PATH:-}" ]; then
        echo "$INSTALL_DIR" >> "$GITHUB_PATH"
        log_info "Appended $INSTALL_DIR to \$GITHUB_PATH"
    fi
}

configure_path

# Optional: Shell Completion Setup
setup_completions() {
    local comp_dir="$HOME/.local/share/bash-completion/completions"
    if [ -d "$comp_dir" ] && [ -w "$comp_dir" ]; then
        "$INSTALL_DIR/$APP" completion bash > "$comp_dir/$APP" 2>/dev/null || true
    fi
}
setup_completions

# Final completion screen
echo -e "\n${GREEN}${BOLD}M31A has been successfully installed!${NC}\n"
echo -e "${MUTED}Installation location: ${NC}$INSTALL_DIR/$APP"
echo -e ""
echo -e "${BOLD}Next Steps:${NC}"
echo -e "  1. Reload your shell configuration or update your current session:"
echo -e "     ${CYAN}source ~/.bashrc${NC}  ${MUTED}# or source ~/.zshrc / restart terminal${NC}"
echo -e "     ${MUTED}or run directly:${NC} ${CYAN}export PATH=\"$INSTALL_DIR:\$PATH\"${NC}"
echo -e ""
echo -e "  2. Run the interactive onboarding setup:"
echo -e "     ${CYAN}m31a setup${NC}        ${MUTED}# Configure NVIDIA NIM credentials & models${NC}"
echo -e ""
echo -e "  3. Validate environment & Bubblewrap sandbox:"
echo -e "     ${CYAN}m31a doctor${NC}       ${MUTED}# Run diagnostic healthcheck${NC}"
echo -e ""
echo -e "  4. Start coding with M31A:"
echo -e "     ${CYAN}m31a${NC}              ${MUTED}# Launch interactive TUI session${NC}"
echo -e "     ${CYAN}m31a \"fix bug\"${NC}     ${MUTED}# Autonomous agent execution${NC}"
echo -e ""
echo -e "${MUTED}To uninstall and clean all data at any time, run: ${NC}${CYAN}m31a-uninstall${NC} ${MUTED}or${NC} ${CYAN}$HOME/.m31a/uninstall.sh${NC}\n"
