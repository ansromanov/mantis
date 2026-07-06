class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.15.10"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "39f2efc9da4a919a37335632d2865084ec7309aca6b764d310ee0edea31b908b"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "104fad25ec8b76f75a6745b98faf955edf1463b5c940887d2742969472e9c174"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "6a61b7719542c6526b82a78f1f0e955f9bc1581ba437869825f33a0298083453"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "469ce45fc32a67fed55b73012937412cd6c0eaef2f45309b8d7a8903509a5caa"
    end
  end

  def install
    bin.install Dir["mantis-*"].first => "mantis"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mantis --version")
  end
end
