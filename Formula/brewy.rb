class Brewy < Formula
  desc "Fast terminal interface for Homebrew"
  homepage "https://github.com/Hexadecimall/homebrew-brewy"
  url "https://github.com/Hexadecimall/homebrew-brewy.git",
      tag:      "v0.1.1",
      revision: "6feb4f8b31e5f02e97a3d263be9a62e518f86bf7"
  license "MIT"
  head "https://github.com/Hexadecimall/homebrew-brewy.git", branch: "main"

  bottle do
    root_url "https://raw.githubusercontent.com/Hexadecimall/homebrew-brewy/main/bottles"
    sha256 cellar: :any_skip_relocation, arm64_golden_gate: "04ed4bf68b91fc113870b56e49d890429e64b57392e7185320b4165ab2921dea"
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
