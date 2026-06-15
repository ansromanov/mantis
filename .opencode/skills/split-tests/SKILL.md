---
name: split-tests
description: Split inline #[cfg(test)] mod tests { ... } blocks into co-located _test.rs files
---

Always split inline `#[cfg(test)] mod tests { ... }` blocks into co-located `_test.rs` files.

## What to do

For each Rust source file `src/foo.rs` that contains an inline test block:
1. Create `src/foo_test.rs` with `use super::*;` at the top, followed by the bare test functions (no `mod tests { }` wrapper)
2. In `src/foo.rs`, replace the inline block with:
   ```rust
   #[cfg(test)]
   #[path = "foo_test.rs"]
   mod tests;
   ```

For directory modules like `src/app/mod.rs`, the test file is `src/app/mod_test.rs`.

## When to use me

Use this skill when:
- You encounter a Rust source file with `#[cfg(test)] mod tests { ... }` inline — immediately split it
- You create a new Rust module — immediately create its `_test.rs` companion
- You add tests to an existing module — append to the existing `_test.rs`, never inline
