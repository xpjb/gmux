#!/bin/bash

# A script to compile and install the 'gmux' Rust project.
#
# USAGE:
#   ./install.sh         # Installs the program
#   ./install.sh uninstall # Uninstalls the program

# --- Configuration ---
# The name of the executable binary.
EXEC_NAME="gmux"
# The installation directory. /usr/local/bin is standard for user-installed executables.
INSTALL_DIR="/usr/local/bin"
# The full path to the final installed executable.
INSTALL_PATH="$INSTALL_DIR/$EXEC_NAME"

# --- Functions ---

# Function to print a formatted message.
# $1: Message type (e.g., INFO, ERROR)
# $2: Message text
function log() {
    case "$1" in
        INFO)  echo "[INFO] $2" ;;
        ERROR) echo "[ERROR] $2" >&2 ;;
        SUCCESS) echo "✅ $2" ;;
    esac
}

# Function to handle the installation process.
function install_app() {
    log INFO "Starting installation for $EXEC_NAME..."

    # 1. Check for dependencies (cargo).
    if ! command -v cargo &> /dev/null; then
        log ERROR "Rust and Cargo are not installed. Please install the Rust toolchain first."
        log INFO "Visit https://rustup.rs/ for installation instructions."
        exit 1
    fi
    log INFO "Rust toolchain found."

    # 2. Compile the project in release mode for performance.
    # The `set -e` command at the top will cause the script to exit if this fails.
    log INFO "Compiling project in release mode... (This may take a moment)"
    cargo build --release

    # 3. Install the binary.
    # We use the `install` command as it's more robust than `cp`. It can set
    # permissions and create destination directories if they don't exist.
    # This requires sudo privileges.
    log INFO "Installing binary to $INSTALL_PATH..."
    if sudo install -Dm755 "target/release/$EXEC_NAME" "$INSTALL_PATH"; then
        log SUCCESS "$EXEC_NAME was successfully installed. You can now run it from anywhere."
    else
        log ERROR "Installation failed. Could not copy binary to $INSTALL_PATH."
        log INFO "Please check your permissions."
        exit 1
    fi
}

# Function to handle the uninstallation process.
function uninstall_app() {
    log INFO "Starting uninstallation for $EXEC_NAME..."

    # 1. Check if the file exists.
    if [ ! -f "$INSTALL_PATH" ]; then
        log ERROR "Executable not found at $INSTALL_PATH. Nothing to uninstall."
        exit 1
    fi

    # 2. Remove the binary.
    # This requires sudo privileges.
    log INFO "Removing executable from $INSTALL_PATH..."
    if sudo rm "$INSTALL_PATH"; then
        log SUCCESS "$EXEC_NAME was successfully uninstalled."
    else
        log ERROR "Uninstallation failed. Could not remove binary from $INSTALL_PATH."
        log INFO "Please check your permissions."
        exit 1
    fi
}

# --- Main Script Logic ---

# Exit immediately if a command exits with a non-zero status.
set -e

# Check the first argument to see if we are installing or uninstalling.
if [ "$1" == "uninstall" ]; then
    uninstall_app
else
    install_app
fi

