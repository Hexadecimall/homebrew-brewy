class Brewy < Formula
  desc "Fast terminal interface for Homebrew"
  homepage "https://github.com/Hexadecimall/homebrew-brewy"
  url "https://github.com/Hexadecimall/homebrew-brewy.git",
      tag:      "v0.2.1",
      revision: "8152b5d213b488cc5aceff086a711a3fd21bf30f"
  license "MIT"
  head "https://github.com/Hexadecimall/homebrew-brewy.git", branch: "main"

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
