class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.15.11"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "c3aa0872f9ef120ad6189fac944aeed67cff05e46be4b89a4d2b718c47306445"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "e1d7ec3545c86384944118244fcc7cb9ffa4c485712264446a044eb74e1717f0"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "63365a351a8ccf9142181d9c20cc118c213f507301a870eae75990e4d02cf09e"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "c5ff6d1fbe0b4affa7e09d218227c02fa97b4bf765734b0000ac7b0372c0d2de"
    end
  end

  def install
    bin.install Dir["mantis-*"].first => "mantis"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mantis --version")
  end
end
