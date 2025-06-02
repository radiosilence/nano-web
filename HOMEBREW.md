# Getting nano-web on Homebrew

This guide covers how to distribute nano-web through Homebrew, both via a custom tap and eventually to homebrew-core.

## Option 1: Create Your Own Homebrew Tap (Recommended First Step)

### 1. Create a New Repository

Create a new GitHub repository named `homebrew-nano-web` (the `homebrew-` prefix is required).

```bash
# Create the repository on GitHub, then clone it
git clone https://github.com/radiosilence/homebrew-nano-web.git
cd homebrew-nano-web
```

### 2. Create the Formula Directory Structure

```bash
mkdir Formula
```

### 3. Create the Formula File

Create `Formula/nano-web.rb`:

```ruby
class NanoWeb < Formula
  desc "Hyper-minimal, lightning-fast web server for SPAs and static content"
  homepage "https://github.com/radiosilence/nano-web"
  url "https://github.com/radiosilence/nano-web/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "YOUR_SHA256_HERE"
  license "MIT"
  head "https://github.com/radiosilence/nano-web.git", branch: "main"

  depends_on "go" => :build

  def install
    ldflags = %W[
      -s -w
      -X github.com/radiosilence/nano-web/version.Version=#{version}
    ]
    
    system "go", "build", *std_go_args(ldflags: ldflags)
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nano-web version")
    assert_match "nano-web", shell_output("#{bin}/nano-web --help")
    assert_match "serve", shell_output("#{bin}/nano-web serve --help")
  end

  def caveats
    <<~EOS
      To start nano-web:
        nano-web serve [directory]
      
      For SPA mode:
        nano-web serve --spa-mode [directory]
      
      See 'nano-web --help' for all options.
    EOS
  end
end
```

### 4. Get the SHA256 Hash

```bash
# Download the release tarball and get its SHA256
curl -L https://github.com/radiosilence/nano-web/archive/refs/tags/v0.2.0.tar.gz | shasum -a 256
```

Update the `sha256` field in the formula with this value.

### 5. Test the Formula Locally

```bash
# Install from local formula
brew install --build-from-source ./Formula/nano-web.rb

# Test it works
nano-web version
nano-web --help

# Uninstall for testing
brew uninstall nano-web
```

### 6. Commit and Push

```bash
git add Formula/nano-web.rb
git commit -m "Add nano-web formula"
git push origin main
```

## Option 2: Automate Updates with GitHub Actions

Create `.github/workflows/update-formula.yml` in your `homebrew-nano-web` repository:

```yaml
name: Update Formula

on:
  repository_dispatch:
    types: [update-formula]
  workflow_dispatch:
    inputs:
      version:
        description: 'Version to update to (e.g., v0.2.1)'
        required: true
      sha256:
        description: 'SHA256 of the release tarball'
        required: true

jobs:
  update-formula:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Update formula
        run: |
          VERSION="${{ github.event.inputs.version || github.event.client_payload.version }}"
          SHA256="${{ github.event.inputs.sha256 || github.event.client_payload.sha256 }}"
          
          # Remove 'v' prefix if present
          VERSION_NUMBER=${VERSION#v}
          
          # Update the formula file
          sed -i "s|archive/refs/tags/v[^/]*\.tar\.gz|archive/refs/tags/$VERSION.tar.gz|" Formula/nano-web.rb
          sed -i "s/sha256 \"[^\"]*\"/sha256 \"$SHA256\"/" Formula/nano-web.rb
          
          # Commit changes
          git config --local user.email "action@github.com"
          git config --local user.name "GitHub Action"
          git add Formula/nano-web.rb
          git commit -m "Update nano-web to $VERSION" || exit 0
          git push
```

### 7. Trigger Updates from Main Repository

Add this step to your main nano-web release workflow (`.github/workflows/release.yml`):

```yaml
  update-homebrew:
    needs: release
    runs-on: ubuntu-latest
    steps:
      - name: Get release info
        id: release_info
        run: |
          VERSION="${{ steps.version.outputs.version }}"
          # Download and get SHA256 of the source tarball
          curl -L "https://github.com/radiosilence/nano-web/archive/refs/tags/$VERSION.tar.gz" -o source.tar.gz
          SHA256=$(sha256sum source.tar.gz | cut -d' ' -f1)
          echo "sha256=$SHA256" >> $GITHUB_OUTPUT

      - name: Update Homebrew formula
        uses: peter-evans/repository-dispatch@v2
        with:
          token: ${{ secrets.HOMEBREW_UPDATE_TOKEN }}
          repository: radiosilence/homebrew-nano-web
          event-type: update-formula
          client-payload: |
            {
              "version": "${{ steps.version.outputs.version }}",
              "sha256": "${{ steps.release_info.outputs.sha256 }}"
            }
```

You'll need to create a Personal Access Token with `repo` permissions and add it as `HOMEBREW_UPDATE_TOKEN` in your main repository's secrets.

## How Users Install

Once your tap is set up, users can install nano-web like this:

```bash
# Add your tap
brew tap radiosilence/nano-web

# Install nano-web
brew install nano-web

# Or in one command
brew install radiosilence/nano-web/nano-web
```

## Option 3: Submit to Homebrew Core (Official)

After your tap is stable and has users, you can submit to homebrew-core:

### Requirements for homebrew-core:
1. **Stable, well-known software** ✅ (nano-web fits this)
2. **Maintained and stable** ✅ (your project appears well-maintained)
3. **Significant user base** (you'll need to build this first with your tap)
4. **Not duplicating existing functionality** ✅ (while there are other static servers, nano-web has unique features)
5. **Open source license** ✅ (MIT license)

### Steps to submit:
1. Fork `homebrew/homebrew-core`
2. Add your formula to `Formula/nano-web.rb`
3. Test thoroughly
4. Submit a pull request
5. Respond to maintainer feedback

### Formula for homebrew-core:

The formula would be similar but potentially simpler:

```ruby
class NanoWeb < Formula
  desc "Hyper-minimal, lightning-fast web server for SPAs and static content"
  homepage "https://github.com/radiosilence/nano-web"
  url "https://github.com/radiosilence/nano-web/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "YOUR_SHA256_HERE"
  license "MIT"

  depends_on "go" => :build

  def install
    ldflags = %W[
      -s -w
      -X github.com/radiosilence/nano-web/version.Version=#{version}
    ]
    
    system "go", "build", *std_go_args(ldflags: ldflags)
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nano-web version")
    assert_match "serve", shell_output("#{bin}/nano-web serve --help")
  end
end
```

## Tips for Success

1. **Start with your own tap** - Build a user base and prove stability
2. **Keep formula simple** - Homebrew prefers minimal, clean formulas  
3. **Good test coverage** - Your tests should verify the binary works
4. **Regular updates** - Keep your tap updated with new releases
5. **Documentation** - Make it easy for users to find and use your tap

## Marketing Your Tap

Once your tap is ready:

1. Update your main README with installation instructions
2. Add Homebrew installation to your documentation
3. Tweet about it / share on social media
4. Consider adding a badge: `[![Homebrew](https://img.shields.io/badge/homebrew-available-brightgreen)](https://github.com/radiosilence/homebrew-nano-web)`

## Maintenance

- Monitor for new releases and update your formula
- Respond to issues in your tap repository  
- Consider automation for formula updates
- Eventually, once stable, submit to homebrew-core for wider distribution

Your project looks perfect for Homebrew distribution - it's a useful CLI tool, well-documented, and has proper releases. Start with your own tap and work toward homebrew-core submission!