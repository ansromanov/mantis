class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.22.3"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "a7cc10cc59a36f7e2a800831ba2b9eecff5387e32acb1cd8922e03d86863c46c"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "ac7d4ec090ecece70268cdbeaa4af90a5ef5fd9a0a23e1405531bb9899d1f985"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "ec3fcde7210fc30a5a0d31c62f8bb08a2140409e982b8e6c0e9e6ab684f6ae43"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "8db8ae766f251bb69883d654b4e5c9aebcbc905dd164cfa8ae96e23d6da5b9eb"
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
