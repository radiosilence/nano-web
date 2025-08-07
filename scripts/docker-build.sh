#!/bin/bash

# Multi-architecture Docker build script for nano-web
# Builds optimized images for multiple platforms

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔥 NANO-WEB MULTI-ARCH DOCKER BUILD${NC}"
echo "===================================="

# Get version from VERSION file
VERSION=$(cat VERSION 2>/dev/null || echo "dev")
echo -e "${GREEN}Version: $VERSION${NC}"

# Docker registry and image name
REGISTRY=${REGISTRY:-""}
IMAGE_NAME=${IMAGE_NAME:-"nano-web"}
FULL_IMAGE="${REGISTRY}${IMAGE_NAME}"

echo -e "${BLUE}Image: $FULL_IMAGE${NC}"
echo

# Platforms to build for
PLATFORMS="linux/amd64,linux/arm64"

# Check if buildx is available
if ! docker buildx version >/dev/null 2>&1; then
    echo -e "${RED}❌ Docker buildx is required but not available${NC}"
    echo "Install buildx: https://docs.docker.com/buildx/working-with-buildx/"
    exit 1
fi

# Create and use a new builder instance
BUILDER_NAME="nano-web-builder"
echo -e "${YELLOW}Creating buildx builder: $BUILDER_NAME${NC}"
docker buildx create --name "$BUILDER_NAME" --use --bootstrap >/dev/null 2>&1 || true
docker buildx use "$BUILDER_NAME"

# Verify the builder supports our target platforms
echo -e "${YELLOW}Checking platform support...${NC}"
if docker buildx inspect --bootstrap | grep -q "linux/amd64.*linux/arm64\|linux/arm64.*linux/amd64"; then
    echo -e "${GREEN}✓ Multi-platform support verified${NC}"
else
    echo -e "${RED}❌ Builder doesn't support required platforms${NC}"
    exit 1
fi

echo

# Build arguments
BUILD_ARGS=(
    "--platform" "$PLATFORMS"
    "--file" "Dockerfile.multi"
    "--tag" "$FULL_IMAGE:$VERSION"
    "--tag" "$FULL_IMAGE:latest"
)

# Add push flag if PUSH=true
if [[ "${PUSH:-false}" == "true" ]]; then
    BUILD_ARGS+=("--push")
    echo -e "${YELLOW}Will push to registry after build${NC}"
else
    BUILD_ARGS+=("--load")
    echo -e "${YELLOW}Will load to local Docker (single platform)${NC}"
fi

# Add cache options for faster builds
BUILD_ARGS+=(
    "--cache-from" "type=gha"
    "--cache-to" "type=gha,mode=max"
)

echo -e "${BLUE}Building for platforms: $PLATFORMS${NC}"
echo -e "${BLUE}Build command:${NC}"
echo "docker buildx build ${BUILD_ARGS[*]} ."
echo

# Perform the build
if docker buildx build "${BUILD_ARGS[@]}" .; then
    echo
    echo -e "${GREEN}🎉 BUILD SUCCESSFUL${NC}"
    echo "================================"
    echo -e "${GREEN}✓ Built for: $PLATFORMS${NC}"
    echo -e "${GREEN}✓ Tags: $FULL_IMAGE:$VERSION, $FULL_IMAGE:latest${NC}"
    
    if [[ "${PUSH:-false}" == "true" ]]; then
        echo -e "${GREEN}✓ Pushed to registry${NC}"
    else
        echo -e "${YELLOW}⚠️  Images built but not pushed (set PUSH=true to push)${NC}"
    fi
    
    echo
    echo -e "${BLUE}Usage examples:${NC}"
    echo "docker run --rm -p 3000:3000 -v \$(pwd)/public:/public $FULL_IMAGE:$VERSION"
    echo "docker run --rm -p 3000:3000 -v \$(pwd)/public:/public $FULL_IMAGE:latest --ultra"
    echo
    echo -e "${BLUE}Image sizes:${NC}"
    if [[ "${PUSH:-false}" != "true" ]]; then
        docker images "$FULL_IMAGE" --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"
    fi
else
    echo
    echo -e "${RED}❌ BUILD FAILED${NC}"
    exit 1
fi

# Cleanup builder (optional)
if [[ "${CLEANUP:-true}" == "true" ]]; then
    echo -e "${YELLOW}Cleaning up builder...${NC}"
    docker buildx rm "$BUILDER_NAME" >/dev/null 2>&1 || true
fi

echo
echo -e "${GREEN}Build complete! 🚀${NC}"