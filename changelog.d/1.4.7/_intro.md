Released 2026-09-29.

### Changed

- **macOS release binaries are signed with a Developer ID and notarised.** Downloaded through a browser, the ad-hoc signed binaries were quarantined and Gatekeeper refused to run them until the quarantine attribute was cleared.

### Changed

- **CI**: The repository had no caches at all. GitHub evicts an entry after 7 days without a read and nano-web goes quiet for longer than that, so every run started from a cold `main` and recompiled the whole dependency graph. The workflow now also runs on a twice-weekly schedule; a run restores each cache, and a restore counts as a read.
- **CI**: `build-binaries`, `build-image` and `publish-crate` only save caches on `main`, which `test` and `lint` already did. Six branch-scoped entries per pull request — none of them readable from any other branch — were evicting `main`'s from the repository's shared 10GB.
- **CI**: `test`, `lint` and `format` are one `check` job. Clippy compiles the dependency graph and the tests reuse it, where two jobs each built it into a cache of its own.
- **CI**: Builds pass `--locked`. A resolver that is free to move produces a `Cargo.lock` that differs from the committed one, and the lockfile hash is part of the cache key.
- **CI**: `fail-fast: false` on the build matrices. A cancelled leg saves nothing, so one broken target left the rest cold on the next run too.
