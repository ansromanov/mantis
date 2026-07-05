class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.15.0"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "a6d7396cc36ad487eeceb5eaca79509bc320bb044e49c2927362d8a0b52e25d7"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "3fe487b2ea6eefeead72cf701201a2974d6fd56cc611d30f6d843e4ddc708159"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "45c0c0d0de122ad5e144c766a43f716421492382424ca55a13fc828bf3c4b8c6"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "aec6c1b26626b1b01612f9059a1a013ec4661649632a6b36aee1da5dde10613b"
    end
  end

  def install
    bin.install Dir["mantis-*"].first => "mantis"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mantis --version")
  end
end
