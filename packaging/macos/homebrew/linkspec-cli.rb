# Homebrew formula for the CLI only. Maintainer: fill sha256.
class LinkspecCli < Formula
  desc "USB-C, Thunderbolt and USB4 diagnosis on the command line"
  homepage "https://github.com/itsh-neumeier/linkspec"
  url "https://github.com/itsh-neumeier/linkspec/releases/download/v0.9.1/linkspec-0.9.1-macos-universal.tar.gz"
  sha256 "ca3338a93a9aa2dba53cac0dca5e70663b67fe01f48e99bd795f51540a42054e"
  license "MIT"

  def install
    bin.install "linkspec"
  end

  test do
    assert_match "linkspec", shell_output("#{bin}/linkspec --version")
  end
end
