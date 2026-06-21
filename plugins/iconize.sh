#!/usr/bin/env bash
# Bundled plugin: provides Nerd Font file-type icon glyphs for the tree.
#
# On `init`, sends a `set_icon_map` action with a glyph-per-extension mapping
# so tv can render icons before each tree entry. Requires a Nerd Font in your
# terminal.
#
# Install: enable in tv.toml:
#   icons = true
#   [plugins]
#   iconize = { path = "iconize.sh", enabled = true }

set -euo pipefail

# Build the icon map once on startup and send it to tv.
send_icon_map() {
  # Directory icons
  local dir_open dir_closed fallback
  dir_open=$(printf '\uf07c')   # nf-fa-folder_open
  dir_closed=$(printf '\uf07b') # nf-fa-folder
  fallback=$(printf '\uf15b')   # nf-fa-file_code

  # Language icons (Devicons / Nerd Font)
  local rs py js ts go java c cpp cs zig
  rs=$(printf '\ue7a8')    # nf-dev-rust
  py=$(printf '\ue73c')    # nf-dev-python
  js=$(printf '\ue74e')    # nf-dev-javascript
  ts=$(printf '\ue628')    # nf-dev-typescript
  go=$(printf '\ue627')    # nf-dev-go
  java=$(printf '\ue738')  # nf-dev-java
  c=$(printf '\ue79b')     # nf-dev-c
  cpp=$(printf '\ue61d')   # nf-dev-cplusplus
  cs=$(printf '\ue77f')    # nf-dev-csharp
  zig=$(printf '\ue6a9')   # nf-dev-zig

  local rb php lua hs swift kotlin dart elixir clj erl scala r
  rb=$(printf '\ue739')    # nf-dev-ruby
  php=$(printf '\ue73d')   # nf-dev-php
  lua=$(printf '\ue620')   # nf-dev-lua
  hs=$(printf '\ue61f')    # nf-dev-haskell
  swift=$(printf '\ue755') # nf-dev-swift
  kotlin=$(printf '\ue634') # nf-dev-kotlin
  dart=$(printf '\ue798')  # nf-dev-dart
  elixir=$(printf '\ue62d') # nf-dev-elixir
  clj=$(printf '\ue768')   # nf-dev-clojure
  erl=$(printf '\ue7b1')   # nf-dev-erlang
  scala=$(printf '\ue737') # nf-dev-scala
  r=$(printf '\ue71c')     # nf-dev-r

  # Web / markup
  local html css scss less vue svelte
  html=$(printf '\ue736')  # nf-dev-html5
  css=$(printf '\ue749')   # nf-dev-css3
  scss=$(printf '\ue74b')  # nf-dev-sass
  less=$(printf '\ue758')  # nf-dev-less
  vue=$(printf '\ue6d0')   # nf-dev-vue
  svelte=$(printf '\ue698') # nf-dev-svelte

  # Config / data
  local json yaml toml sql graphql docker
  json=$(printf '\ue60b')    # nf-dev-json
  yaml=$(printf '\ue73a')    # nf-dev-yaml
  toml=$(printf '\ue60b')    # no dedicated toml icon; reuse json glyph
  sql=$(printf '\ue706')     # nf-dev-database
  graphql=$(printf '\ue844') # nf-dev-graphql
  docker=$(printf '\ue7b0')  # nf-dev-docker

  # Shell / scripts
  local sh bash zsh
  sh=$(printf '\ue795')    # nf-dev-terminal
  bash=$(printf '\ue795')  # nf-dev-terminal
  zsh=$(printf '\ue795')   # nf-dev-terminal

  # Other
  local md lock exe img vid aud pdf archive font node
  md=$(printf '\ue73e')     # nf-dev-markdown
  lock=$(printf '\ue6c6')   # nf-dev-lock
  exe=$(printf '\ue70f')    # nf-dev-terminal_badge
  img=$(printf '\uf1c5')    # nf-fa-image
  vid=$(printf '\uf03d')    # nf-fa-video
  aud=$(printf '\uf001')    # nf-fa-music
  pdf=$(printf '\uf1c1')    # nf-fa-file_pdf
  archive=$(printf '\uf1c6') # nf-fa-file_archive
  font=$(printf '\uf031')   # nf-fa-font
  node=$(printf '\ue718')   # nf-dev-nodejs_small

  # Assemble the JSON payload (use %s for printf-safe values).
  # The { and } are literal; variables contain the UTF-8 glyph bytes.
  printf '{"event":"action","action":"set_icon_map","params":{"dir_open":"%s","dir_closed":"%s","fallback":"%s","icons":{"rs":"%s","py":"%s","js":"%s","ts":"%s","go":"%s","java":"%s","c":"%s","cpp":"%s","cs":"%s","zig":"%s","rb":"%s","php":"%s","lua":"%s","hs":"%s","swift":"%s","kt":"%s","kts":"%s","dart":"%s","ex":"%s","clj":"%s","cljs":"%s","erl":"%s","scala":"%s","r":"%s","rmd":"%s","html":"%s","htm":"%s","css":"%s","scss":"%s","less":"%s","vue":"%s","svelte":"%s","json":"%s","yaml":"%s","yml":"%s","toml":"%s","sql":"%s","db":"%s","sqlite":"%s","graphql":"%s","gql":"%s","dockerfile":"%s","sh":"%s","bash":"%s","zsh":"%s","fish":"%s","md":"%s","markdown":"%s","lock":"%s","exe":"%s","bin":"%s","so":"%s","dll":"%s","dylib":"%s","png":"%s","jpg":"%s","jpeg":"%s","gif":"%s","svg":"%s","ico":"%s","webp":"%s","mp4":"%s","avi":"%s","mkv":"%s","mov":"%s","mp3":"%s","wav":"%s","ogg":"%s","flac":"%s","m4a":"%s","pdf":"%s","epub":"%s","mobi":"%s","zip":"%s","tar":"%s","gz":"%s","xz":"%s","bz2":"%s","7z":"%s","rar":"%s","ttf":"%s","otf":"%s","woff":"%s","woff2":"%s","eot":"%s","node":"%s"}}}\n' \
    "$dir_open" "$dir_closed" "$fallback" \
    "$rs" "$py" "$js" "$ts" "$go" \
    "$java" "$c" "$cpp" "$cs" "$zig" \
    "$rb" "$php" "$lua" "$hs" "$swift" \
    "$kotlin" "$kotlin" "$dart" "$elixir" "$clj" "$clj" \
    "$erl" "$scala" "$r" "$r" \
    "$html" "$html" "$css" "$scss" "$less" \
    "$vue" "$svelte" \
    "$json" "$yaml" "$yaml" "$toml" \
    "$sql" "$sql" "$sql" "$graphql" "$graphql" \
    "$docker" \
    "$sh" "$bash" "$zsh" "$sh" \
    "$md" "$md" "$lock" \
    "$exe" "$exe" "$exe" "$exe" "$exe" \
    "$img" "$img" "$img" "$img" "$img" "$img" "$img" \
    "$vid" "$vid" "$vid" "$vid" \
    "$aud" "$aud" "$aud" "$aud" "$aud" \
    "$pdf" "$pdf" "$pdf" \
    "$archive" "$archive" "$archive" "$archive" "$archive" "$archive" "$archive" \
    "$font" "$font" "$font" "$font" "$font" \
    "$node"
}

while IFS= read -r line; do
  event="${line#*\"event\":\"}"
  event="${event%%\"*}"

  case "$event" in
    init)
      send_icon_map
      ;;
    shutdown)
      exit 0
      ;;
  esac
done
