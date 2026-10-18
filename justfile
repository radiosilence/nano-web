# nano-web — static file server with pre-compressed in-memory caching

# What CI checks: format, lint with warnings as errors, tests.
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features --locked -- -D warnings
    cargo test --all-features --locked

# Format
fmt:
    cargo fmt --all

# Install the git hooks
hooks:
    lefthook install

# Write CHANGELOG.md from the fragments in changelog.d/. `--stdout` prints it instead.
changelog *args:
    python3 scripts/changelog.py {{args}}

# Open the release PR: `just release patch|minor|major`, or an exact version.
# Merging it publishes the release; tags are never pushed by hand.
release version *args:
    scripts/release.sh {{version}} {{args}}
