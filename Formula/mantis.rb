class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.16.13"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "960996acff2e1c426ff6f70deafb1a88292f2c50b20437bcbdb9c725c5f57e6d"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "07995bfa4576d463c440d0d9eff18d5509a18657f1d797ffe9eb7fd43f1c3902"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "8e6a94b64e1b27a842762e908421b970aba1ba2a0e2452343394171ec10ffc89"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "3cfbf2bb1f6e758b06559574185b51d5f2bd516465edad4832fa973577d51fab"
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
