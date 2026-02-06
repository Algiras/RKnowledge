#!/bin/bash
# RKnowledge Skill Installation Script
# Delegates to the main install.sh for platform detection and binary download.
set -e

REPO="Algiras/RKnowledge"

# Delegate to the main installer
if command -v curl &>/dev/null; then
    curl -fsSL "https://raw.githubusercontent.com/${REPO}/main/install.sh" | bash -s -- "$@"
elif command -v wget &>/dev/null; then
    wget -qO- "https://raw.githubusercontent.com/${REPO}/main/install.sh" | bash -s -- "$@"
else
    echo "Error: curl or wget is required"
    exit 1
fi
