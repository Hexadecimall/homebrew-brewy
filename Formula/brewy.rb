class Brewy < Formula
  desc "Fast terminal interface for Homebrew"
  homepage "https://github.com/Hexadecimall/homebrew-brewy"
  url "https://github.com/Hexadecimall/homebrew-brewy.git",
      tag:      "v0.2.1",
      revision: "8152b5d213b488cc5aceff086a711a3fd21bf30f"
  license "MIT"
  head "https://github.com/Hexadecimall/homebrew-brewy.git", branch: "main"

  bottle do
    root_url "https://raw.githubusercontent.com/Hexadecimall/homebrew-brewy/main/bottles"
    sha256 cellar: :any_skip_relocation, arm64_golden_gate: "a272f17e54c682f1c51ce98592b4e160dd01924e6f3ba3c26c096eec2bd7d481"
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
