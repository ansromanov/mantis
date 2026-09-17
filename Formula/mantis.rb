class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.22.5"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "74dff3de2adfb13392621859219768325cf37c42248a02832a061746adedd558"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "a72698b2cf08e7041fc72513e63c4058973a522af9c9b084b6225192e340c955"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "1bb5e403543c8372c2fca7d0cbc7eee8e88d4a96590e4d9fb115e78fc0fd59a3"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "3fdaab1fd0118aa0a005ee71ac8fc02c6b18d196e86f27ee95d48c071807a074"
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
