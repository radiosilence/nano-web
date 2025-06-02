# Getting nano-web on Homebrew

This guide covers how to distribute nano-web through Homebrew using the formula in your main repository.

## Formula in Main Repository (Recommended Approach)

Your nano-web formula lives right in your main repository at `Formula/nano-web.rb`. This approach:

- ✅ **Keeps everything together** - Formula stays in sync with code
- ✅ **Easier maintenance** - One repo to manage  
- ✅ **Simpler for users** - Direct install from main repo
- ✅ **Better CI/CD** - Update formula in same workflow as releases

## How Users Install

### Direct Install (Recommended)
```bash
brew install radiosilence/nano-web/nano-web
```

### Alternative: Using Tap
```bash
brew tap radiosilence/nano-web https://github.com/radiosilence/nano-web.git
brew install nano-web
```

## The Formula File

Your `Formula/nano-web.rb` contains:

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

## Get the SHA256 Hash

```bash
# Download the release tarball and get its SHA256
curl -L https://github.com/radiosilence/nano-web/archive/refs/tags/v0.2.0.tar.gz | shasum -a 256
```

Update the `sha256` field in the formula with this value.

## Test the Formula Locally

```bash
# Install from local formula
brew install --build-from-source ./Formula/nano-web.rb

# Test it works
nano-web version
nano-web --help

# Uninstall for testing
brew uninstall nano-web
```

## Automate Updates with GitHub Actions

Add this step to your existing release workflow (`.github/workflows/release.yml`):

```yaml
  update-homebrew:
    needs: release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Get release info
        id: release_info
        run: |
          VERSION="${{ steps.version.outputs.version }}"
          # Download and get SHA256 of the source tarball
          curl -L "https://github.com/radiosilence/nano-web/archive/refs/tags/$VERSION.tar.gz" -o source.tar.gz
          SHA256=$(sha256sum source.tar.gz | cut -d' ' -f1)
          echo "sha256=$SHA256" >> $GITHUB_OUTPUT

      - name: Update Homebrew formula
        run: |
          VERSION="${{ steps.version.outputs.version }}"
          SHA256="${{ steps.release_info.outputs.sha256 }}"
          
          # Update the formula file
          sed -i "s|archive/refs/tags/v[^/]*\.tar\.gz|archive/refs/tags/$VERSION.tar.gz|" Formula/nano-web.rb
          sed -i "s/sha256 \"[^\"]*\"/sha256 \"$SHA256\"/" Formula/nano-web.rb
          
          # Commit changes
          git config --local user.email "action@github.com"
          git config --local user.name "GitHub Action"
          git add Formula/nano-web.rb
          git commit -m "Update Homebrew formula to $VERSION" || exit 0
          git push
```

## Submit to Homebrew Core (Official)

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

1. **Keep formula simple** - Homebrew prefers minimal, clean formulas  
2. **Good test coverage** - Your tests should verify the binary works
3. **Automate updates** - Use GitHub Actions to update formula on releases
4. **Documentation** - Make it easy for users to find installation instructions

## Marketing Your Formula

Once your formula is ready:

1. Update your main README with installation instructions
2. Add Homebrew installation to your documentation  
3. Tweet about it / share on social media
4. Consider adding a badge: `[![Homebrew](https://img.shields.io/badge/homebrew-available-brightgreen)](https://github.com/radiosilence/nano-web)`

## Maintenance

- Formula updates automatically via GitHub Actions on releases
- Respond to issues in your main repository
- Eventually, once stable, submit to homebrew-core for wider distribution

Your project looks perfect for Homebrew distribution - it's a useful CLI tool, well-documented, and has proper releases with the formula right in your main repo!