#!/usr/bin/env bash

set -e

echo "=========================================="
echo "Building Modern Blockchain System"
echo "=========================================="

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Build Rust workspace
echo -e "${BLUE}Building Rust workspace...${NC}"
cargo build --workspace --release
echo -e "${GREEN}✓ Rust workspace built successfully${NC}"

# Build Elixir networking layer
echo -e "${BLUE}Building Elixir networking layer...${NC}"
(cd networking && mix deps.get && mix compile)
echo -e "${GREEN}✓ Elixir networking layer built successfully${NC}"

echo ""
echo -e "${GREEN}=========================================="
echo "Build completed successfully!"
echo "==========================================${NC}"
