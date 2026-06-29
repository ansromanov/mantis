class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.11.24"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "73c15aa3a00fc59c30d6dc8806b640cd9d30e97796924dc423966ac263f07024"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "fa41563409abeb35e581516b7c38244017ef95516d0407380cf9aaa0f63e7f43"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "fd93b8496cb3d468d5cb5a869fdf6e49f4bd104a8d97f3c4a90191af229931c2"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "410e839489effe146c7e4fe470dcfb01523a3792e6e185c8a25d2f04a4ee7d3e"
    end
  end

  def install
    bin.install Dir["mantis-*"].first => "mantis"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mantis --version")
  end
end
