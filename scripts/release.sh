#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/release.sh [patch|minor|major]
LEVEL=${1:-patch}

echo "🚀 Starting release process for level: $LEVEL"

# Make sure we're on main and up to date
if [ "$(git branch --show-current)" != "main" ]; then
    echo "❌ Must be on main branch"
    exit 1
fi

echo "📥 Pulling latest changes..."
git pull origin main

# Run tests first
echo "🧪 Running tests..."
cargo test

# Dry run first
echo "🔍 Dry run release..."
cargo release $LEVEL --dry-run

echo ""
echo "This will:"
echo "  1. Bump version in Cargo.toml and VERSION file"
echo "  2. Update CHANGELOG.md"  
echo "  3. Create a commit"
echo "  4. Create and push a git tag"
echo "  5. Trigger the release workflow which will publish to crates.io"
echo ""

read -p "Continue with release? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "❌ Release cancelled"
    exit 1
fi

# Do the actual release - this will commit, tag, and push
echo "📦 Creating release..."
cargo release $LEVEL --execute

echo "✅ Release complete! CI will handle publishing to crates.io"
echo "🔗 Check the release workflow: https://github.com/$(git config --get remote.origin.url | sed 's/.*github.com[:/]\([^.]*\).*/\1/')/actions"