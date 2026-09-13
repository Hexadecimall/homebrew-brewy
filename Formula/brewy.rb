class Brewy < Formula
  desc "Fast terminal interface for Homebrew"
  homepage "https://github.com/Hexadecimall/homebrew-brewy"
  url "https://github.com/Hexadecimall/homebrew-brewy.git",
      tag:      "v0.2.0",
      revision: "6395d05ce5bb66659929793493ea16e94da58ea7"
  license "MIT"
  head "https://github.com/Hexadecimall/homebrew-brewy.git", branch: "main"

  bottle do
    root_url "https://raw.githubusercontent.com/Hexadecimall/homebrew-brewy/main/bottles"
    sha256 cellar: :any_skip_relocation, arm64_golden_gate: "bb5107f5e2d68bb4158c555fa847e2d41aa3abd19b1d247b8b06ac59512da733"
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
