# Markdown Feature Showcase

This document demonstrates the markdown elements that **mantis** renders.
Open it in `mantis` and press `M` to toggle between the rendered and raw views.
Each section below focuses on one family of elements so you can compare the
source against the rendered output side by side.

> **Tip:** Press `z` to toggle word wrap, and use `j`/`k` (or the arrow keys)
> to scroll the content panel once it is focused with `Tab`.

## Headings

Headings give a document its outline. The heading above (`Markdown Feature
Showcase`) is an `H1`, and the one introducing this section is an `H2`. The
renderer styles each level differently — color and weight change as you go
deeper — so the hierarchy stays readable even in a terminal.

### Heading Level 3

Third-level headings are good for sub-topics within a section, such as the
individual list flavors shown later in this document.

#### Heading Level 4
##### Heading Level 5
###### Heading Level 6

Levels four through six share a style here, but they still create structure in
the underlying outline and in the raw source.

## Text Styling

Prose carries most of a document's meaning, so inline styling matters. You can
write **bold text** for emphasis, *italic text* for subtle stress, and
***bold and italic*** when you need both at once. Use ~~strikethrough~~ to show
removed or outdated content, and `inline code spans` for identifiers, file
names like `src/config.rs`, or short commands such as `cargo build`.

Styles nest and combine, so a phrase like ***bold italic wrapping `inline
code`*** renders every modifier together. This is handy when documenting an
API where a term is both important and a literal symbol.

A soft line break in the source
continues on the same rendered line, which keeps paragraphs reflowable.
A hard break, made with two trailing spaces,\
forces the text onto a new line — useful for addresses or verse where the line
boundaries are meaningful.

## Lists

Lists organize related items. The renderer supports both unordered and ordered
lists, and they can nest to arbitrary depth.

### Unordered

- First item, a plain top-level entry
- Second item, with children
  - Nested item A, indented one level
  - Nested item B
    - Deeply nested item, two levels in
  - Nested item C, back out one level
- Third item, back at the top level

### Ordered

1. Clone the repository with `git clone`
2. Build the project:
   1. Run `cargo build` for a debug binary
   2. Run `cargo build --release` for an optimized one
3. Launch the viewer with `mantis .`
4. Press `?` at any time to open the in-app help

## Blockquotes

Blockquotes set apart cited or secondary material, and they can nest to show
threaded context such as a reply to a reply.

> A single-level quote introduces an idea worth highlighting.
>
> It can span multiple paragraphs while staying visually distinct.
>
> > A nested quote sits inside the first one.
> > This is useful for email-style replies or layered citations.
> >
> > > A third level goes even deeper when the conversation calls for it.

## Code Block

Fenced code blocks preserve whitespace and are framed in the rendered view so
they stand out from prose. Here is a small Rust program:

```rust
fn main() {
    let greeting = "Hello, mantis!";
    // Split on whitespace and print each word on its own line.
    for (index, word) in greeting.split_whitespace().enumerate() {
        println!("{index}: {word}");
    }
}
```

A configuration sample in TOML, mirroring the options this viewer reads:

```toml
# Layout and behavior
show_hidden = true
ignore_gitignore = true
tree_width = 20
word_wrap = true

[keys]
quit = ["q", "ctrl+c"]
toggle_wrap = ["z"]
```

And a shell snippet showing typical usage:

```sh
# View the current directory, then a specific file
mantis .
mantis example.md
```

## Table

Tables line up structured data into columns. The renderer measures each column
and supports per-column alignment, shown below as left, center, and right.

| Feature        | Key       | Default | Status      |
| :------------- | :-------: | ------: | :---------- |
| Quit           | `q`       |     yes | done        |
| Toggle wrap    | `z`       |      no | done        |
| Toggle hidden  | `alt+.`   |      no | done        |
| Search files   | `/`       |     n/a | done        |
| Search content | `f`       |     n/a | done        |
| Reload tree    | `r`       |     n/a | done        |
| Mouse support  | —         |      no | planned     |

The first column is left-aligned, the key column is centered, the default
column is right-aligned, and the status column is left-aligned again.

## Task List / Checkboxes

Task lists track progress. Checked items render with a filled box (☑) and
unchecked items with an empty one (☐), making a roadmap easy to scan.

- [x] Configurable tree width via `tree_width`
- [x] Toggleable word wrap, on by default in this repo
- [x] Remappable keybindings through the `[keys]` table
- [x] Project-local config that overrides the global one
- [ ] Mouse support for click-to-select
- [ ] Horizontal split layout
- [ ] Syntax-aware folding in the content panel

## Links and Images

Links keep references close to the text that needs them. Visit the
[project repository](https://example.com/mantis) for source and issues,
or read the [configuration guide](https://example.com/mantis/config) for
the full list of options. The rendered view shows the link text; the raw view
reveals the underlying URL.

Images are represented by a compact placeholder so the layout stays intact in a
text terminal: ![architecture diagram](https://example.com/diagram.png) sits
inline right here.

## Horizontal Rule

A horizontal rule marks a strong topic break — a shift in subject rather than
just a new paragraph.

Content above the rule wraps up the previous thought.

---

Content below the rule begins something new, signaling to the reader that the
preceding section is complete.
