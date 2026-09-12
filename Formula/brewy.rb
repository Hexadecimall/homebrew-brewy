class Brewy < Formula
  desc "Fast terminal interface for Homebrew"
  homepage "https://github.com/Hexadecimall/homebrew-brewy"
  url "https://github.com/Hexadecimall/homebrew-brewy.git",
      tag:      "v0.1.0",
      revision: "f2a351f0829b953414d7f64b53d3553a88d0c8c5"
  license "MIT"
  head "https://github.com/Hexadecimall/homebrew-brewy.git", branch: "main"

  bottle do
    root_url "https://raw.githubusercontent.com/Hexadecimall/homebrew-brewy/main/bottles"
    sha256 cellar: :any_skip_relocation, arm64_golden_gate: "42ff632fd138e8089af984657e1035d0f3d335f4d3b5ac9320861fe05a4ca8b8"
  end

  depends_on "rust" => :build
  depends_on :macos

  def install
    rustflags = [
      "--remap-path-prefix=#{buildpath}=.",
      "--remap-path-prefix=#{HOMEBREW_CACHE}=/homebrew-cache",
      "--remap-path-prefix=#{HOMEBREW_PREFIX}/Cellar/rust=/homebrew-rust",
    ]
    ENV["CARGO_ENCODED_RUSTFLAGS"] = rustflags.join("\x1f")

    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match "brewy #{version}", shell_output("#{bin}/brewy --version")
  end
end
