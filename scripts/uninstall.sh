#!/usr/bin/env bash
# ==============================================================================
# M31A Uninstaller for Linux
# Cleanly terminates M31A processes and completely purges binaries,
# local configs, databases, sessions, caches, and shell integrations.
# https://github.com/eshanized/M31A
# ==============================================================================

set -euo pipefail

APP="m31a"
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

log_action() {
    echo -e "${CYAN}[-]${NC} $1"
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

print_banner() {
    echo -e "${RED}"
    cat << 'EOF'
  ███╗   ███╗██████╗  ██╗ █████╗ 
  ████╗ ████║╚════██╗███║██╔══██╗
  ██╔████╔██║ █████╔╝╚██║███████║
  ██║╚██╔╝██║ ╚═══██╗ ██║██╔══██║
  ██║ ╚═╝ ██║██████╔╝ ██║██║  ██║
  ╚═╝     ╚═╝╚═════╝  ╚═╝╚═╝  ╚═╝
EOF
    echo -e "${NC}${BOLD}M31A Complete Uninstaller & Purge Utility${NC}\n"
}

usage() {
    cat <<EOF
M31A Uninstaller for Linux

Completely uninstalls M31A and cleans all local data, configurations,
databases, session records, daemon sockets, caches, and shell PATH modifications.

Usage: uninstall.sh [options]

Options:
    -h, --help          Display this help message
    -f, -y, --force     Skip confirmation prompts and force non-interactive uninstall
        --dry-run       Preview all files, directories, processes, and PATH entries to be removed
        --keep-config   Preserve configuration files (~/.m31a/config.toml, ~/.config/m31a)
        --keep-data     Preserve persistent databases, session history, and memories
    -d, --dir <path>    Specify custom installation binary directory to check

Examples:
    ./uninstall.sh
    ./uninstall.sh --force
    ./uninstall.sh --dry-run
    ./uninstall.sh --keep-config
EOF
}

# Parse options
force=false
dry_run=false
keep_config=false
keep_data=false
custom_dir=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            usage
            exit 0
            ;;
        -f|-y|--force|--yes)
            force=true
            shift
            ;;
        --dry-run)
            dry_run=true
            shift
            ;;
        --keep-config)
            keep_config=true
            shift
            ;;
        --keep-data)
            keep_data=true
            shift
            ;;
        -d|--dir|--install-dir)
            if [[ -n "${2:-}" ]]; then
                custom_dir="$2"
                shift 2
            else
                log_error "--dir requires a directory path"
                exit 1
            fi
            ;;
        *)
            log_warning "Unknown option '$1'"
            shift
            ;;
    esac
done

print_banner

# Safe path check to prevent accidental disaster
is_safe_to_remove() {
    local target="$1"
    if [ -z "$target" ]; then
        return 1
    fi
    # Never remove / or /home or root $HOME directly
    if [ "$target" = "/" ] || [ "$target" = "/root" ] || [ "$target" = "/home" ] || [ "$target" = "$HOME" ]; then
        return 1
    fi
    # Check that it contains "m31a" in the path name as safety barrier
    if [[ "$target" != *"m31a"* ]]; then
        return 1
    fi
    return 0
}

# Helper to remove directory safely
safe_remove_dir() {
    local dir="$1"
    local desc="${2:-Directory}"
    if [ -d "$dir" ]; then
        if ! is_safe_to_remove "$dir"; then
            log_error "Safety refusal: target directory '$dir' is not an M31A path!"
            return 1
        fi

        if [ "$dry_run" = "true" ]; then
            echo -e "  ${ORANGE}[DRY-RUN] Would purge ${desc}:${NC} $dir"
        else
            log_action "Purging ${desc}: $dir"
            rm -rf "$dir"
        fi
    fi
}

# Helper to remove file safely
safe_remove_file() {
    local file="$1"
    local desc="${2:-File}"
    if [ -f "$file" ] || [ -L "$file" ] || [ -S "$file" ]; then
        if [ "$dry_run" = "true" ]; then
            echo -e "  ${ORANGE}[DRY-RUN] Would remove ${desc}:${NC} $file"
        else
            log_action "Removing ${desc}: $file"
            rm -f "$file"
        fi
    fi
}

# 1. Collect targets for review
XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"

TARGET_CONFIG_DIR="$XDG_CONFIG_HOME/m31a"
TARGET_M31A_HOME="$HOME/.m31a"
TARGET_M31A_VAULT="$HOME/.m31a-vault"
TARGET_DATA_DIR="$XDG_DATA_HOME/m31a"
TARGET_CACHE_DIR="$XDG_CACHE_HOME/m31a"
TARGET_STATE_DIR="$XDG_STATE_HOME/m31a"

# Interactive confirmation
if [ "$force" != "true" ] && [ "$dry_run" != "true" ]; then
    echo -e "${BOLD}The following M31A resources will be terminated and wiped:${NC}"
    echo -e "  - Running M31A background daemons and tasks"
    echo -e "  - Binary executables in ~/.m31a/bin, ~/.local/bin, etc."
    if [ "$keep_config" = "false" ]; then
        echo -e "  - ${RED}ALL local configs & credentials${NC} (~/.m31a, ~/.config/m31a)"
    else
        echo -e "  - (Preserving configuration files: ~/.config/m31a, config.toml)"
    fi
    if [ "$keep_data" = "false" ]; then
        echo -e "  - ${RED}ALL session history, memories & SQLite databases${NC} (state.db, storage.db)"
        echo -e "  - ${RED}Encrypted security vault${NC} (~/.m31a-vault)"
    else
        echo -e "  - (Preserving databases & session data)"
    fi
    echo -e "  - Caches and temporary runtime sockets"
    echo -e "  - Shell PATH configurations (.bashrc, .zshrc, .config/fish/config.fish)"
    echo -e ""

    # Check if stdin is interactive or /dev/tty is available
    if [ -t 0 ]; then
        read -r -p "Are you sure you want to completely uninstall M31A? [y/N] " response
    elif [ -r /dev/tty ]; then
        read -r -p "Are you sure you want to completely uninstall M31A? [y/N] " response < /dev/tty
    else
        log_error "Non-interactive environment detected without --force flag. Aborting."
        echo -e "${MUTED}Run with: ./uninstall.sh --force${NC}"
        exit 1
    fi

    case "$response" in
        [yY][eE][sS]|[yY])
            echo ""
            ;;
        *)
            log_info "Uninstallation cancelled by user. No files were modified."
            exit 0
            ;;
    esac
fi

# ==============================================================================
# Step 1: Terminate active background daemon processes
# ==============================================================================
log_info "Checking for active M31A daemon processes..."

stop_daemon_pid_file() {
    local pid_file="$1"
    if [ -f "$pid_file" ]; then
        local pid
        pid=$(cat "$pid_file" 2>/dev/null || true)
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            if [ "$dry_run" = "true" ]; then
                echo -e "  ${ORANGE}[DRY-RUN] Would terminate daemon process PID:${NC} $pid"
            else
                log_action "Sending SIGTERM to M31A daemon process PID: $pid"
                kill "$pid" 2>/dev/null || true
                local count=0
                while kill -0 "$pid" 2>/dev/null && [ "$count" -lt 30 ]; do
                    sleep 0.1
                    count=$((count + 1))
                done
                if kill -0 "$pid" 2>/dev/null; then
                    log_warning "Daemon PID $pid did not exit cleanly; sending SIGKILL..."
                    kill -9 "$pid" 2>/dev/null || true
                fi
            fi
        fi
        safe_remove_file "$pid_file" "Daemon PID file"
    fi
}

stop_daemon_pid_file "$TARGET_M31A_HOME/m31a-daemon.pid"
stop_daemon_pid_file "$TARGET_CONFIG_DIR/m31a-daemon.pid"

# Kill any other running m31a daemons
if command -v pgrep >/dev/null 2>&1; then
    running_pids=$(pgrep -f "m31a daemon" 2>/dev/null || true)
    if [ -n "$running_pids" ]; then
        for p in $running_pids; do
            if [ "$dry_run" = "true" ]; then
                echo -e "  ${ORANGE}[DRY-RUN] Would terminate lingering daemon PID:${NC} $p"
            else
                log_action "Stopping lingering daemon process PID: $p"
                kill "$p" 2>/dev/null || true
            fi
        done
    fi
fi

# Remove sockets
safe_remove_file "$TARGET_M31A_HOME/m31a-daemon.sock" "Daemon socket"
safe_remove_file "$TARGET_CONFIG_DIR/m31a-daemon.sock" "Daemon socket"
for sock in /tmp/m31a*.sock /tmp/m31a-daemon*; do
    if [ -e "$sock" ]; then
        safe_remove_file "$sock" "Temporary socket/file"
    fi
done

# ==============================================================================
# Step 2: Remove Binary Executables & Completions
# ==============================================================================
log_info "Removing M31A executable binaries..."

BIN_CANDIDATES=(
    "$INSTALL_DIR/m31a"
    "$INSTALL_DIR/m31a-uninstall"
    "$HOME/.m31a/bin/m31a"
    "$HOME/.m31a/bin/m31a-uninstall"
    "$HOME/.local/bin/m31a"
    "$HOME/.local/bin/m31a-uninstall"
    "$HOME/.cargo/bin/m31a"
)

if [ -n "$custom_dir" ]; then
    BIN_CANDIDATES+=("$custom_dir/m31a" "$custom_dir/m31a-uninstall")
fi

for bin_path in "${BIN_CANDIDATES[@]}"; do
    if [ -f "$bin_path" ] || [ -L "$bin_path" ]; then
        safe_remove_file "$bin_path" "M31A binary/shim"
    fi
done

# System binary location (/usr/local/bin or /usr/bin)
for sys_bin in "/usr/local/bin/m31a" "/usr/local/bin/m31a-uninstall" "/usr/bin/m31a"; do
    if [ -f "$sys_bin" ]; then
        if [ -w "$sys_bin" ] || [ "$EUID" -eq 0 ]; then
            safe_remove_file "$sys_bin" "System M31A binary"
        else
            log_warning "Found system binary at $sys_bin, but cannot remove without sudo privileges. Run: sudo rm -f $sys_bin"
        fi
    fi
done

# Shell completions
safe_remove_file "$HOME/.local/share/bash-completion/completions/m31a" "Bash completion"
safe_remove_file "$HOME/.bash_completion.d/m31a" "Bash completion"
safe_remove_file "$HOME/.zfunc/_m31a" "Zsh completion"
safe_remove_file "$HOME/.config/fish/completions/m31a.fish" "Fish completion"

# ==============================================================================
# Step 3: Clean Configurations, Databases, and Local Data
# ==============================================================================
log_info "Purging local data, databases, and cache..."

# Cache
safe_remove_dir "$TARGET_CACHE_DIR" "Cache directory"
for tmp_install in /tmp/m31a_install_*; do
    if [ -d "$tmp_install" ]; then
        safe_remove_dir "$tmp_install" "Temporary install directory"
    fi
done

# Data & State
if [ "$keep_data" = "false" ]; then
    safe_remove_dir "$TARGET_DATA_DIR" "XDG Data directory"
    safe_remove_dir "$TARGET_STATE_DIR" "XDG State directory"
    safe_remove_dir "$TARGET_M31A_VAULT" "Vault credentials & keyrings"
else
    log_info "Preserving local databases and vault data (--keep-data)"
fi

# Config directory (~/.config/m31a)
if [ "$keep_config" = "false" ]; then
    safe_remove_dir "$TARGET_CONFIG_DIR" "XDG Config directory"
else
    log_info "Preserving XDG config directory (--keep-config)"
fi

# Custom M31A_HOME if set
if [ -n "${M31A_HOME:-}" ] && [ "$M31A_HOME" != "$HOME" ]; then
    if [ "$keep_config" = "false" ] && [ "$keep_data" = "false" ]; then
        safe_remove_dir "$M31A_HOME" "Custom M31A_HOME directory"
    fi
fi

# User home root (~/.m31a)
if [ -d "$TARGET_M31A_HOME" ]; then
    if [ "$keep_config" = "false" ] && [ "$keep_data" = "false" ]; then
        safe_remove_dir "$TARGET_M31A_HOME" "Main M31A data directory (~/.m31a)"
    else
        log_info "Cleaning non-preserved files in $TARGET_M31A_HOME..."
        if [ "$dry_run" = "true" ]; then
            echo -e "  ${ORANGE}[DRY-RUN] Would clean contents of $TARGET_M31A_HOME while keeping selected files${NC}"
        else
            # Remove bin, sockets, logs, cache, temporary items
            rm -rf "$TARGET_M31A_HOME/bin" "$TARGET_M31A_HOME/plugins" "$TARGET_M31A_HOME/feedback" "$TARGET_M31A_HOME"/*.sock "$TARGET_M31A_HOME"/*.pid "$TARGET_M31A_HOME/uninstall.sh"
            if [ "$keep_data" = "false" ]; then
                rm -rf "$TARGET_M31A_HOME/sessions" "$TARGET_M31A_HOME/memories" "$TARGET_M31A_HOME/worktrees" "$TARGET_M31A_HOME"/*.db*
            fi
            if [ "$keep_config" = "false" ]; then
                rm -rf "$TARGET_M31A_HOME"/*.toml "$TARGET_M31A_HOME/credentials.json" "$TARGET_M31A_HOME/trusted_dirs.json"
            fi
        fi
    fi
fi

# System configuration in /etc/m31a
if [ -d "/etc/m31a" ]; then
    if [ -w "/etc/m31a" ] || [ "$EUID" -eq 0 ]; then
        safe_remove_dir "/etc/m31a" "System configuration directory"
    else
        log_warning "Found system config at /etc/m31a, but cannot remove without sudo privileges. Run: sudo rm -rf /etc/m31a"
    fi
fi

# ==============================================================================
# Step 4: Revert Shell Configuration (PATH removals)
# ==============================================================================
log_info "Cleaning PATH configuration from shell rc files..."

clean_shell_rc() {
    local rc_file="$1"
    [ -f "$rc_file" ] || return 0

    local modified=false

    # Check for marked initialization block
    if grep -q "m31a path initialization" "$rc_file" 2>/dev/null; then
        if [ "$dry_run" = "true" ]; then
            echo -e "  ${ORANGE}[DRY-RUN] Would remove M31A path block from:${NC} $rc_file"
            modified=true
        else
            log_action "Removing M31A initialization block from: $rc_file"
            local tmp_rc
            tmp_rc=$(mktemp)
            awk '
                BEGIN { skipping = 0; prev_blank = 0 }
                /# >>> m31a path initialization >>>/ { skipping = 1; next }
                /# <<< m31a path initialization <<</ { skipping = 0; next }
                skipping { next }
                /^[[:space:]]*$/ {
                    if (!prev_blank) { print "" }
                    prev_blank = 1
                    next
                }
                {
                    prev_blank = 0
                    print
                }
            ' "$rc_file" > "$tmp_rc"
            mv "$tmp_rc" "$rc_file"
            modified=true
        fi
    fi

    # Check for any stray export PATH entries mentioning .m31a/bin
    if grep -E 'export PATH=.*\.m31a/bin' "$rc_file" >/dev/null 2>&1; then
        if [ "$dry_run" = "true" ]; then
            echo -e "  ${ORANGE}[DRY-RUN] Would remove legacy PATH export from:${NC} $rc_file"
        else
            log_action "Removing legacy PATH entry from: $rc_file"
            local tmp_rc
            tmp_rc=$(mktemp)
            grep -v -E 'export PATH=.*\.m31a/bin' "$rc_file" > "$tmp_rc" || true
            mv "$tmp_rc" "$rc_file"
            modified=true
        fi
    fi

    # Check for fish path additions
    if grep -E 'fish_add_path .*\.m31a/bin' "$rc_file" >/dev/null 2>&1; then
        if [ "$dry_run" = "true" ]; then
            echo -e "  ${ORANGE}[DRY-RUN] Would remove fish_add_path entry from:${NC} $rc_file"
        else
            log_action "Removing fish_add_path entry from: $rc_file"
            local tmp_rc
            tmp_rc=$(mktemp)
            grep -v -E 'fish_add_path .*\.m31a/bin' "$rc_file" > "$tmp_rc" || true
            mv "$tmp_rc" "$rc_file"
            modified=true
        fi
    fi

    if [ "$modified" = "true" ] && [ "$dry_run" = "false" ]; then
        log_success "Cleaned shell config: $rc_file"
    fi
}

RC_FILES=(
    "$HOME/.bashrc"
    "$HOME/.bash_profile"
    "$HOME/.profile"
    "${ZDOTDIR:-$HOME}/.zshrc"
    "${ZDOTDIR:-$HOME}/.zshenv"
    "$HOME/.config/fish/config.fish"
    "$HOME/.ashrc"
)

for rc in "${RC_FILES[@]}"; do
    clean_shell_rc "$rc"
done

# ==============================================================================
# Summary
# ==============================================================================
echo ""
if [ "$dry_run" = "true" ]; then
    echo -e "${CYAN}${BOLD}[DRY-RUN COMPLETE]${NC} No modifications were made to your filesystem."
else
    echo -e "${GREEN}${BOLD}M31A has been completely uninstalled.${NC}"
    echo -e "${MUTED}All binaries, local configurations, databases, caches, and shell integrations have been removed.${NC}"
    echo -e "${MUTED}To reinstall in the future, run:${NC}"
    echo -e "  ${CYAN}curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash${NC}\n"
fi
