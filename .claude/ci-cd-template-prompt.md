# CI/CD Pipeline Template — Rust + GitHub Actions

You are helping set up a GitHub Actions CI/CD pipeline for a Rust project. Use the design described below as the reference implementation. Before generating the workflow file, ask the user the questions at the bottom.

---

## Design principles

- **Version detection uses GitHub Releases API** — never git tags as input. Tags are write-only output created by the release job. This avoids false positives on forks or repos with stale/imported tags.
- **Release is triggered by bumping the version in `Cargo.toml`** and merging to main. No manual tagging, no release branches.
- **PRs only run CI** (test/lint/format). All release and Docker jobs are explicitly gated on `github.ref == 'refs/heads/main' && github.event_name == 'push'` — not implicitly via skipped deps.
- **Binary builds and Docker image builds are skipped entirely on non-release main pushes** — no wasted CI minutes.
- **Concurrency**: PRs cancel in-progress runs on new push; main pushes never cancel.
- **Node.js 24** opted in globally via `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` env var.

---

## Job flow

### Always on PR and main push
- `test` — `cargo test --all-features`
- `lint` — `cargo clippy --all-targets --all-features -- -D warnings`
- `format` — `cargo fmt --all -- --check`

### Main push only
- `check-version` — reads `Cargo.toml` version, queries GitHub Releases API for latest release tag, outputs `should_release`, `version`, `major`, `major_minor`

### Main push, only when `should_release == true`
- `build-binaries` (matrix) — cross-compiles for linux-amd64-gnu, linux-arm64-gnu, linux-amd64-musl, linux-arm64-musl, macos-arm64, windows-amd64. Uploads artifacts.

### Docker jobs (main push only, if Docker enabled)
- `build-image` (matrix: amd64 + arm64) — always on main push, pushes `main-{arch}` tag. On release, also retags as `{version}-{arch}` via `docker buildx imagetools create`.
- `create-manifest` — always on main push (needs test/lint/format + build-image). Pushes `:main` multi-arch manifest always. On release, also pushes `:latest`, `:{version}`, `:{major}`, `:{major}.{minor}`, `:{short-sha}`.
- `quick-test` — pulls `:main`, spins up container, hits `/_health` and checks response content.

### Release jobs (only when `should_release == true`)
- `create-release` — needs test + lint + format + build-binaries + check-version + quick-test (if Docker). Creates annotated git tag, creates GitHub Release with auto-generated notes, attaches all binary artifacts.
- `publish-crate` — needs create-release. Runs `cargo publish` with `CARGO_TOKEN` secret.

---

## Key implementation details

**check-version script:**
```bash
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
LATEST_RELEASE=$(gh release list --limit 1 --json tagName -q '.[0].tagName' 2>/dev/null || echo "")
LATEST_VERSION=${LATEST_RELEASE#v}
if [ -z "$LATEST_RELEASE" ] || [ "$CURRENT_VERSION" != "$LATEST_VERSION" ]; then
  echo "should_release=true" >> $GITHUB_OUTPUT
else
  echo "should_release=false" >> $GITHUB_OUTPUT
fi
```
Requires `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` in env.

**Docker arch images use `ubuntu-24.04-arm` runner for arm64** — native, not QEMU emulation.

**musl builds** require `musl-tools` apt package installed before building.

**Binary artifact naming**: `{binary-name}-{os}-{arch}[-{libc}][.exe]`

**Secrets required:**
- `GITHUB_TOKEN` — auto-provided, used for GHCR and GH releases
- `CARGO_TOKEN` — crates.io API token (only needed if publishing to crates.io)

---

## Questions to ask the user before generating

1. **What is the binary name?** (used in artifact naming and build paths)
2. **Do you want Docker image publishing to GHCR?** (adds build-image, create-manifest, quick-test jobs)
   - If yes: what health check endpoint does the container expose? (default: `/_health`)
   - If yes: what port does it listen on? (default: `3000`)
   - If yes: is there a directory to mount as a volume for the smoke test? (e.g. `public/`)
3. **Do you want to publish to crates.io?** (adds publish-crate job, requires CARGO_TOKEN secret)
4. **Which target platforms for binaries?** (default: linux amd64/arm64 gnu+musl, macos arm64, windows amd64 — remove any not needed)
5. **Any additional CI steps needed?** (e.g. integration tests, benchmarks, coverage)
