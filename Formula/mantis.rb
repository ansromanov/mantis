class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.24.0"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "fed54ce0907fcbbc844a0f732929b79c59d3d2fe15f07ff882219f12c51686fa"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "04f0267c53efec3cd4ac4a4e9cada2243cd1837724d37efc1492089d655ff016"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "c08636fb967861f5102a6f216f7c713ed887186633701bfe8a716e66b1b13f96"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "2280fe3f7135fa77f71706f3b60b295df495516e4c7d9e75a198687bc5ba881b"
    end
  end

  def install
    bin.install Dir["mantis-*"].first => "mantis"

    # Generate shell completions and man page from the installed binary.
    %w[bash zsh fish].each do |shell|
      (buildpath/"mantis.#{shell}").write Utils.popen_read(bin/"mantis", "--completions", shell)
    end
    bash_completion.install "mantis.bash"
    zsh_completion.install "mantis.zsh" => "_mantis"
    fish_completion.install "mantis.fish"

    (buildpath/"mantis.1").write Utils.popen_read(bin/"mantis", "--print-man-page")
    man1.install "mantis.1"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mantis --version")
    # Completions and man page should be non-empty.
    assert_match "mantis", shell_output("#{bin}/mantis --completions bash")
    assert_match ".TH", shell_output("#{bin}/mantis --print-man-page")
  end
end
