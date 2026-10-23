# Homebrew formula for the CLI only. Maintainer: fill sha256.
class LinkspecCli < Formula
  desc "USB-C, Thunderbolt and USB4 diagnosis on the command line"
  homepage "https://github.com/itsh-neumeier/linkspec"
  url "https://github.com/itsh-neumeier/linkspec/releases/download/v0.9.2/linkspec-0.9.2-macos-universal.tar.gz"
  sha256 "2f3787e9e343cae1438a6b2dbb9dbbfec6445a5e60dba8a2d015046ce4af9e56"
  license "MIT"

  def install
    bin.install "linkspec"
  end

  test do
    assert_match "linkspec", shell_output("#{bin}/linkspec --version")
  end
end
