class NanoWeb < Formula
  desc "Hyper-minimal, lightning-fast web server for SPAs and static content"
  homepage "https://github.com/radiosilence/nano-web"
  version "0.8.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/radiosilence/nano-web/releases/download/v0.8.0/nano-web-darwin-amd64.tar.gz"
      sha256 "d278ae3c9cb46c6dc76e95b6b5dba8df21b1ac5723f2493263eb8a1a7a41e03f"
    end
    if Hardware::CPU.arm?
      url "https://github.com/radiosilence/nano-web/releases/download/v0.8.0/nano-web-darwin-arm64.tar.gz"
      sha256 "464610af97df49d0fd6d4c05f824d364a1efbee2eba8ab5140bf335415eaf9c1"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/radiosilence/nano-web/releases/download/v0.8.0/nano-web-linux-amd64.tar.gz"
      sha256 "07c0d5ed03d4f5036a202259852ae048859535393d549b464206e02b78fcbdb8"
    end
    if Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
      url "https://github.com/radiosilence/nano-web/releases/download/v0.8.0/nano-web-linux-arm64.tar.gz"
      sha256 "1c51cc2f013a3054e6cffab1b88397f119c8168910f1bf27b923be2baa092857"
    end
  end

  def install
    if OS.mac?
      if Hardware::CPU.intel?
        bin.install "nano-web-darwin-amd64" => "nano-web"
      elsif Hardware::CPU.arm?
        bin.install "nano-web-darwin-arm64" => "nano-web"
      end
    elsif OS.linux?
      if Hardware::CPU.intel?
        bin.install "nano-web-linux-amd64" => "nano-web"
      elsif Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
        bin.install "nano-web-linux-arm64" => "nano-web"
      end
    end
  end

  test do
    # Test version command
    assert_match version.to_s, shell_output("#{bin}/nano-web version")
    
    # Test help command
    assert_match "nano-web", shell_output("#{bin}/nano-web --help")
    
    # Test serve command help
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