# Homebrew cask (tap itsh-neumeier/homebrew-linkspec). Maintainer: fill sha256.
cask "linkspec" do
  version "0.9.1"
  sha256 "79e93d72717974890993fdcf2779f343c5663c0f2e0d884b428a297554627e7d"
  url "https://github.com/itsh-neumeier/linkspec/releases/download/v#{version}/LinkSpec-#{version}.dmg"
  name "LinkSpec"
  desc "USB-C, Thunderbolt and USB4 diagnosis: which link limits the connection"
  homepage "https://github.com/itsh-neumeier/linkspec"
  depends_on macos: ">= :sonoma"
  depends_on arch: :arm64
  app "LinkSpec.app"
  binary "#{appdir}/LinkSpec.app/Contents/MacOS/linkspec"
  postflight do
    system_command "/usr/bin/touch", args: ["#{appdir}/LinkSpec.app/Contents/MacOS/linkspec-packaged"]
  end
  zap trash: ["~/Library/Application Support/LinkSpec"]
end
