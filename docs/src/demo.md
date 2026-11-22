# Demo

A 40-second tour recorded from a real `mantis` session: fuzzy-opening a markdown
file (rendered with the built-in renderer, word-wrapped, toggled raw and back),
a Rust file with code folding and the symbol outline, full-text search across
the tree, and the theme picker.

<link rel="stylesheet" type="text/css" href="https://cdn.jsdelivr.net/npm/asciinema-player@3.8.0/dist/bundle/asciinema-player.css" />
<div id="mantis-demo"></div>
<script src="https://cdn.jsdelivr.net/npm/asciinema-player@3.8.0/dist/bundle/asciinema-player.min.js"></script>
<script>
  AsciinemaPlayer.create('media/demo.cast', document.getElementById('mantis-demo'), {
    cols: 118,
    rows: 32,
    idleTimeLimit: 2,
    fit: 'width',
    poster: 'npt:0:12',
  });
</script>

You can also play the recording in your own terminal with
[asciinema](https://asciinema.org):

```sh
asciinema play media/demo.cast
```

The cast file lives in the repository at
[`media/demo.cast`](https://github.com/ansromanov/mantis/blob/main/media/demo.cast).
