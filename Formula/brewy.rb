class Brewy < Formula
  desc "Fast terminal interface for Homebrew"
  homepage "https://github.com/Hexadecimall/homebrew-brewy"
  url "https://github.com/Hexadecimall/homebrew-brewy.git",
      tag:      "v0.1.0",
      revision: "f2a351f0829b953414d7f64b53d3553a88d0c8c5"
  license "MIT"
  head "https://github.com/Hexadecimall/homebrew-brewy.git", branch: "main"

  depends_on "rust" => :build
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match "brewy #{version}", shell_output("#{bin}/brewy --version")
  end
end
