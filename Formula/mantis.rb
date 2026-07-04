class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.13.4"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "88672b87ed76a069b6d26fe4c4676eeacdde1b1a636fba7ff5bfe16ccaf8bbca"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "fd43bfddeef4804b29b2707aac61494d20291cb46256dc00942bae783351f565"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "c879556fb8f2f6854003cc109d060c1cbb1d44bffeb79cea28d0d3a5faa8101c"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "7b10cf5cf59f7ed1d717d4bd5209d71b0d06da4231b3bbbf00cb497c04c9c332"
    end
  end

  def install
    bin.install Dir["mantis-*"].first => "mantis"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mantis --version")
  end
end
