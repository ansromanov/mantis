# Documentation screenshots

Every screenshot on the docs site is generated from a [vhs](https://github.com/charmbracelet/vhs)
tape in this directory, so the images can be regenerated after a UI change
instead of being re-captured by hand.

```sh
brew install vhs        # once
just docs-screenshots   # renders every tape into media/docs/
```

Each tape drives the **release build** of mantis (`target/release/mantis`,
built by the recipe) against the fixtures in `fixtures/`, then the recipe
copies the final settled frame to `media/docs/<tape-name>.png`.

## Conventions

- One tape per screenshot, named after the image it produces.
- Open files through `Ctrl+T` and a name fragment rather than counting `Down`
  presses — tree order is not something a tape should depend on.
- Put setup (shell scaffolding, throwaway git repositories) inside a
  `Hide`/`Show` block so it never appears in the image.
- Keep `Set Width`/`Set Height`/`Set FontSize` identical across tapes so the
  images sit consistently on the page.
- Fixtures are committed. If a tape needs new data, add it to `fixtures/`
  rather than reaching into the repository's own source, which changes shape
  over time and would make old screenshots misleading.

## tabs.tape

`tabs.tape` renders correctly but its image is **not** currently used on the
docs site: inactive tab labels are drawn in `theme.dim` on a `theme.dim`
background (`src/ui/tabstrip.rs`), so a two-tab screenshot looks like a
one-tab screenshot — see issue #848. Once that is fixed, re-render and put the
figure back in the v0.20 section of `docs/src/releases.md`.

## Why the last frame

vhs renders a video; `Output <name>.png` makes it write one PNG per frame into
a directory. The tapes end with a `Sleep` on the state being demonstrated, so
the final `frame-text-*.png` is that state, fully settled. (`Screenshot` and
GIF output both silently produce nothing with vhs 0.12 + ffmpeg 9 on macOS,
which is why the recipe takes this route.)
