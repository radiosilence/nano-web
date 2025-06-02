class NanoWeb < Formula
  desc "Hyper-minimal, lightning-fast web server for SPAs and static content"
  homepage "https://github.com/radiosilence/nano-web"
  url "https://github.com/radiosilence/nano-web/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256"
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