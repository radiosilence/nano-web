#!/bin/bash
set -e

# Update CHANGELOG.md with new version entry
# Usage: ./scripts/update-changelog.sh [version] [previous_version]

VERSION=${1:-$(cat VERSION 2>/dev/null || echo "dev")}
PREVIOUS_VERSION=${2:-""}

if [ -z "$PREVIOUS_VERSION" ]; then
    # Find the previous version from git tags
    PREVIOUS_VERSION=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null | sed 's/^v//' || echo "")
fi

# Remove 'v' prefix if present
VERSION=${VERSION#v}
PREVIOUS_VERSION=${PREVIOUS_VERSION#v}

CHANGELOG_FILE="CHANGELOG.md"
TEMP_FILE=$(mktemp)

echo "Updating changelog for version $VERSION (previous: $PREVIOUS_VERSION)"

# Get date in YYYY-MM-DD format
DATE=$(date +%Y-%m-%d)

# Generate changelog entry for the new version
generate_changelog_entry() {
    local version=$1
    local prev_version=$2
    local date=$3
    
    echo "## [$version] - $date"
    
    if [ -n "$prev_version" ] && git rev-parse "v$prev_version" >/dev/null 2>&1; then
        # Get commits between versions
        commits=$(git log --oneline --pretty=format:"- %s" "v$prev_version..HEAD" 2>/dev/null | grep -v "Bump version")
        
        if [ -n "$commits" ]; then
            # Categorize changes
            has_added=false
            has_changed=false
            has_fixed=false
            has_removed=false
            has_technical=false
            
            # Check for different types of changes
            while IFS= read -r commit; do
                if echo "$commit" | grep -qi -E "(add|new|introduce|implement)"; then
                    if [ "$has_added" = false ]; then
                        echo "### Added"
                        has_added=true
                    fi
                    echo "$commit"
                elif echo "$commit" | grep -qi -E "(change|update|modify|enhance|improve)"; then
                    if [ "$has_changed" = false ]; then
                        echo "### Changed"
                        has_changed=true
                    fi
                    echo "$commit"
                elif echo "$commit" | grep -qi -E "(fix|resolve|correct|repair)"; then
                    if [ "$has_fixed" = false ]; then
                        echo "### Fixed"
                        has_fixed=true
                    fi
                    echo "$commit"
                elif echo "$commit" | grep -qi -E "(remove|delete|drop)"; then
                    if [ "$has_removed" = false ]; then
                        echo "### Removed"
                        has_removed=true
                    fi
                    echo "$commit"
                else
                    if [ "$has_technical" = false ]; then
                        echo "### Technical"
                        has_technical=true
                    fi
                    echo "$commit"
                fi
            done <<< "$commits"
        else
            echo "### Technical"
            echo "- Version bump without functional changes"
        fi
    else
        echo "### Technical"
        echo "- Initial release or version bump"
    fi
    
    echo ""  # Empty line after entry
}

# Create new changelog with the new entry at the top
{
    # Keep the header
    head -n 5 "$CHANGELOG_FILE"
    
    # Add new version entry
    generate_changelog_entry "$VERSION" "$PREVIOUS_VERSION" "$DATE"
    
    # Add existing content (skip header)
    tail -n +6 "$CHANGELOG_FILE"
} > "$TEMP_FILE"

# Replace the original file
mv "$TEMP_FILE" "$CHANGELOG_FILE"

echo "✅ Updated $CHANGELOG_FILE with version $VERSION"
echo "📝 Entry added for $DATE"

# Show the new entry
echo ""
echo "New changelog entry:"
echo "==================="
head -n 20 "$CHANGELOG_FILE" | tail -n 15