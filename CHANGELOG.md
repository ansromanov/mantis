## [0.25.0] - 2026-11-22

### Bug Fixes

- *(render)* Word wrap and scroll that survive resize; README and housekeeping (#885)
- *(keys)* One source of truth for default keys; real defaults for display toggles (#890)

### Performance

- Block for a whole idle frame in the input wait instead of waking every 16 ms (#887)
## [0.24.0] - 2026-11-22

### Features

- Enforce the Rust file-size limit (#880)
- Derive Mantis release versions from Conventional Commits (#878)
- Revive and simplify mantis (native folds and markdown, no plugin system)
- Bring Python diagnostics and Kubernetes facts into the app
## [0.23.0] - 2026-09-17

### Features

- *(git)* Improve git UX clarity with badges, diff header bar, and mouse controls (#875)
## [0.22.5] - 2026-09-17

### Bug Fixes

- *(workspaces)* Preserve badges and shared layout (#872)
- *(config)* Edit mantis.toml in place, writing only runtime changes (#874)
## [0.22.3] - 2026-09-16

### Features

- *(tabs)* Show open projects as a Workspaces list above the file tree (#866)

### Bug Fixes

- *(content)* Clamp line-number gutter to narrow content panes (#868)
- *(menu)* Open menu bar dropdowns on click, not hover (#865)
## [0.22.0] - 2026-09-15

### Features

- Add opt-in action menu bar (#842)
- Improve tab ergonomics (#843)
- Make status-bar segments clickable (#841)
- Expand context menu targets and actions (#844)
- Statusbar styling config — separators, per-segment color roles, two-row mode (#845)
- Open worktrees in new tabs (#846)
- *(plugin)* Report debounced content cursor (#847)
- Browse branches from status bar (#831) (#855)
- Add provider-backed symbol outlines (#838) (#857)
- Route language providers by filename and shebang (#856)
- *(plugin)* Stream large rendered content (#858)
- *(plugin)* Let plugins contribute context-menu items, status segments, and menu entries (#859)
- *(plugin)* Implement diagnostics capability (#860)

### Bug Fixes

- Support tab strip horizontal scrolling and overflow affordances (#840)
- *(ui)* Restore inactive tab label contrast (#848) (#854)
- Accept multi-document YAML streams (#849) (#853)
- Stop re-statting the filesystem on every tree redraw (#861)
- *(test)* Scope telemetry and update assertions in about popup test (#862)
## [0.21.0] - 2026-09-11

### Features

- Improve overlay and tree UX (#822)

### Bug Fixes

- Restore compare exit and search navigation keys (#803)
- Resolve bug batch 804-816 (#820)
- Close theme picker with one escape (#821)
## [0.20.2] - 2026-09-08

### Features

- Tabs for multiple projects + workspace restore (#797)
- Add bundled ini and sql language providers (#799)
- Capability-sweep skill — parallel multi-agent E2E verification (#798)

### Bug Fixes

- *(capability-sweep)* Isolate config; protect target mantis.toml (#800)
## [0.20.0] - 2026-09-08

### Features

- Add CSS and TOML folding plugins (#793)
- Add TypeScript and template folding plugins (#795)

### Bug Fixes

- Keep worktree switcher responsive (#796)
## [0.19.0] - 2026-09-06

### Features

- Add JSON path breadcrumb and query bar (#758)
- Status_facts plugin capability + bundled k8s manifest plugin (#771)
- Show real image dimensions in the binary-file placeholder (#773)
- Expose cursor line to selection-change plugins (#772)
- Render images inline via the Kitty graphics protocol (#774)
- Add per-workspace file bookmarks (#775)
- Harden plugin registry trust model (#780)
- Bundled terraform/hcl language provider plugin with fold capability (#782) (#791)

### Bug Fixes

- *(plugin)* Ensure safe default plugin installation and sparse config saving (#768)
- Scroll wrapped markdown by visual rows (#781)
## [0.18.0] - 2026-08-28

### Features

- Implement log follow mode and line filtering (#711)
- Add right-click context menu for tree rows and content pane (#720)
- Add nginx.conf / justfile syntax highlighting (#742)
- Render JSONL rows with in-place expansion (#754)
- Add git worktree switcher (#755)
- Follow the newest changed file (#753)
- Render CSV and TSV files as aligned tables with borders (#757)
- Mask credential values by default (#756)

### Bug Fixes

- Ci: gate auto-merge behind a label instead of default-on (#712)
- Stabilize mouse content scrolling (#714)
- Keep scrollbar drags out of text selection (#729)
- Make piped stdin pager interactive (#728)
- Clarify scroll and cursor indicators (#730)
- Handle narrow terminals clearly (#731)
- Correct tree label and history diffs (#743)
- Restore tree expansion after filtering (#745)
- Keep git mode visible in content focus (#747)
- Close blame commit views with one escape (#749)
- Keep search navigation keys typeable (#746)
- Show blame mode in status bar (#748)
- Discard line filters on escape (#750)
- Use friendly labels for palette-only help entries (#751)
- Improve keymap discoverability (#752)
## [0.17.10] - 2026-07-15

### Bug Fixes

- Omit plugin log path from error message when file does not exist (#710)
## [0.17.9] - 2026-07-13

### Features

- Tabbed revision picker (commits/tags/branches) (#707)
- Allow editing the GitHub issue title in bug reports (#708)

### Bug Fixes

- Use bash shell for windows test step (#698)
- Resolve Windows-only test failures (74 -> 0) (#700)
- Bypass markdown plugin in raw mode (#701)
- Resolve remaining Windows test-suite failures (#702)
- Preserve compare-base range status on reload (#706)
## [0.17.3] - 2026-07-11

### Features

- Bundle TOML, TypeScript/TSX, and Dockerfile syntax plugins (#486) (#680)
- Unified quick-open — prefix routing in the palette (#679)
- *(sh)* Bundled shell language provider plugin (#676)
- *(plugin)* Add yaml language provider plugin (#669)
- Palette ranking frecency decay + plugin-contributed commands (register_commands) (#681)
- *(git)* Repo-wide commit log overlay (#682)
- View file at revision — diff ↔ snapshot toggle (#683)
- Add self-hosted runner option to bench.yml (#693)

### Bug Fixes

- Preserve scroll position on view toggles (wrap, pretty JSON, diff SBS) (#672)
- *(blame)* Move blame UI from tree lane to content pane (#671)
- Add missing help-overlay entries for toggle_file_revision and blame_open_commit (#686)
- Update config schema snapshot for keys added in #683 (#689)
- Gate Unix-only test code and fix file_at_rev on Windows (#690)
- Route self-hosted runners by OS label (#691)
- Route release.yml self-hosted jobs to Linux label (#692)
- Respect runner input in release test-windows job (#694)
- Defer release bump push until pipeline is green (#695)
## [0.16.18] - 2026-07-09

### Bug Fixes

- *(git)* Disable optional locks to prevent index.lock races (#662)
- *(bug-report)* Soft-wrap long description lines (#661)
- *(plugin)* Enable all bundled plugins by default (#663)
- Lower default tree width to 20%, add keyboard resize [ and ] (#665) (#666)
## [0.16.13] - 2026-07-08

### Features

- *(tree-filter)* Support regex in tree filter (/) (#654)

### Bug Fixes

- Always reserve fold gutter width to prevent line-number shift between files (#653)
- *(plugin)* Enable iconize plugin by default (#652)

### Performance

- *(theme)* Cache test-env detection in no_color_active (#658)
## [0.16.8] - 2026-07-08

### Features

- *(pr-review)* Make pr-review skill lightweight (#637)
- Open modal with multiline text input for bug report (#638)
- *(bug-report)* Scrollable diagnostic preview + shared ScrollState (#641)
- *(telemetry)* Instrument key/mouse actions, overlays, features, file opens (#640)
- *(editor)* Config override for external editor, probe nano before vim (#644)
- First-run welcome overlay (one-time, dismissible) (#643)
- Shell completions and man page (#642)
## [0.16.0] - 2026-07-07

### Features

- Add E2E and functional testing suite (#611)
- Hide tree arrows when icons active (#619)
- Local telemetry sink and diagnostic bug reports saved to state dir (#621)
- Shared fold-detector lib (brace + indentation) for language plugins (#627)
- Compare mode — browse tree changes against a chosen revision (#626)
- Implement fold detectors and bundled rust plugin (#629)
- *(blame)* Rework blame — dedicated pane for full-file, bottom-bar for single-line (#628)
- Command palette metadata, highlighting, and keybindings (#630)
- *(plugin)* Add python language provider plugin (#631)
- Bundled go plugin — Go language provider (fold capability) (#633)
- *(palette)* Context-aware command palette (#632)

### Bug Fixes

- *(tree)* Prevent tree_up_dir from ascending past initial root (#614)
## [0.15.11] - 2026-07-06

### Features

- *(keys)* Stable, cross-terminal-safe default keybindings (#609)
## [0.15.10] - 2026-07-06

### Features

- Help discoverability hint + docs sync guard (#304) (#556)
- Add command palette and goto line to help overlay (#579)
- Enable bundled markdown plugin by default (#574) (#588)
- Watch and hot-reload config changes on the fly (#592)
- Richer binary-file placeholder — size, type, open-externally (#593)
- Copy line/selection to clipboard in content pane (#595)
- Add light-background terminal detection and NO_COLOR support (#597)

### Bug Fixes

- Make keyboard shortcuts CapsLock-insensitive on Windows (#580)
- Support mouse text selection on plugin-rendered markdown (#578)
- Restore discoverability of markdown raw/rendered toggle (#559) (#581)
- Add issues: write permission to auto-merge workflow (#584)
- Ctrl+Shift keybindings silently broken on legacy terminals (#561) (#586)
- Add prettify_size_limit to avoid loading large JSON/YAML fully into memory (#589)
- Implement pager mode keyboard input on Windows via CONIN$ (#596)
## [0.15.0] - 2026-07-05

### Bug Fixes

- Skip build_visible deep walk when nothing is expanded (#551)
- Keep selection highlight visible on the active line (#552)
- Stop warning on pre-refactor config keys (git_mode, renamed keymap actions) (#555)
- Address PR #551 review comments, delay auto-merge for review (#554)
## [0.14.0] - 2026-07-05

### Features

- Single action registry unifying palette, keymap, and help (#532)
- *(plugin)* Implement protocol v3 (#541)
- Editor-style keymap defaults (VS Code / Sublime conventions) (#542)
- Content search regex / case-sensitive / whole-word toggles (#543)
- Implement structured tabbed help overlay (#546)
- Pager mode — read piped stdin (git diff | mantis) (#547)
- Enable auto-merge by default on PRs (#549)

### Bug Fixes

- Disable alternate-scroll mode to stop wheel-up glitch at top (#529)
- Overwrite stale bundled plugin binaries on upgrade (#536)
- *(markdown-plugin)* Assign current_file so re-render paths actually run (#537)
- / tree filter cannot find files in collapsed subdirectories (#538)
- Stop clipboard tests from writing to the real system clipboard (#544)
- Update main binary tests to editor-style keymap defaults (#545)

### Performance

- Fix bench slicing bug, O(depth^2) tree walk, per-frame guide scan, per-redraw syntax I/O (#531)
## [0.13.4] - 2026-07-04

### Bug Fixes

- Emit shifted alternate char for CSI-u key events with SHIFT (#524)
- Move active_line cursor on content-pane left click (#523)
- Goto-line places the cursor on the target line (#527)
- Add readable light-theme presets to markdown plugin (#526)
## [0.13.0] - 2026-07-04

### Features

- *(help)* Add Git section to the in-app help overlay (#445)
- *(help)* Make help overlay scrollable (#476)
- Live preview in theme picker (#451) (#488)

### Bug Fixes

- Color gitignored files when ignore_gitignore = true (#446) (#447)
- Raise plugin stdout line cap so large rendered docs aren't dropped (#448)
- Clean up retired shell-script plugins on upgrade (#297) (#463)
- Remove redundant root slash in breadcrumb render (#474)
- External file truncation SIGBUS (#459) (#494)
- Prevent UI thread blocking on exact-4096 input bursts (#456) (#493)
- Parse modifier params in CSI ~ and SGR mouse sequences (#455) (#500)
- Use git status --porcelain -z to handle non-ASCII paths correctly (#501)
- Bound blame cache to 16 entries with LRU eviction (#503)
- Install panic hook to restore terminal on panic (#457) (#502)
- Per-root session files to eliminate concurrent-instance race (#504)
- Remove dead markdown-core feature and in-app markdown rendering (#514)
- Embed bundled plugin binaries at compile time (#513)
- Decode multi-byte UTF-8 in RawEventSource for pasted non-ASCII text (#517)
- Capture plugin stderr for crash diagnostics (#515)
## [0.12.0] - 2026-06-29

### Features

- *(packaging)* Add Homebrew formula with auto-update on release (#443)

### Bug Fixes

- Remove line-number gutter from rendered markdown (#439)
- *(statusbar)* Allow hiding segments via explicit allowlist (#435) (#437)
- Smooth markdown scrolling by pre-wrapping paragraphs (#442)
## [0.11.24] - 2026-06-29

### Features

- Configurable status-bar segment alignment (left/right per segment) (#415)
- Enter in tree filter opens selected match (not just close) (#434)

### Bug Fixes

- *(config)* Surface config save failures in status bar (#412)
- Symbol/punctuation key bindings on non-Latin layouts (#379) (#414)
- *(breadcrumb)* Drop cursor subdir from breadcrumb, fix compact .. target (#388) (#416)
## [0.11.6] - 2026-06-28

### Features

- Dedicated ctrl+f key for global fuzzy file-name picker (#409)

### Bug Fixes

- Tree filter Up/Down/PageUp/PageDown navigate between matches (#408)
- PageUp/PageDown move text cursor in content pane (#410)
## [0.11.3] - 2026-06-27

### Bug Fixes

- Guard open_release_url with TTY check to prevent browser focus-steal (#392)
- Generate changelog before build so binary embeds release notes (#394)
- Gutter line numbers desync from content under word wrap (#390) (#395)
## [0.11.0] - 2026-06-27

### Features

- Rank command palette by recency + frequency (pin last-used + top-3 most-used) (#351)
- Blame column shows author+subject and click opens line-blame popup (#348)
- Make active diff source visible + add diff_mode config default (#364)
- Show working tree clean placeholder in git mode (#307) (#371)
- Restore per-file cursor + scroll position when returning to a file (#387)

### Bug Fixes

- Help overlay uses live keymap instead of hardcoded strings (#292) (#341)
- Parse kitty base-layout key from correct CSI-u field (#353) (#362)
- Preserve scroll and in-file search across same-file reload (#361) (#370)
- Report all keys as escape codes so non-Latin letter bindings fire (#378)

### Performance

- Coalesce mouse-wheel event bursts & cache visible highlight (#346) (#350)
- Draw_tree O(view_height) per frame, not O(total_nodes) (#377)
## [0.10.0] - 2026-06-27

### Features

- Config options for untracked/ignored files in git status (#332)
- Rebrand tree-viewer/tv to mantis (#333)
## [0.9.7] - 2026-06-26

### Bug Fixes

- *(event_source)* Decode arrow key release events to stop double movement (#337)
## [0.9.6] - 2026-06-26

### Bug Fixes

- *(event_source)* Correct modifier decode, SGR drag, and arrow key handling (#334)

### Performance

- Fetch git status off the UI thread (no block on toggle/root change) (#328)
## [0.9.4] - 2026-06-26

### Features

- Add pr-review skill and skip CI for agent-only changes (#329)
- Make active line cursor visible with full-width highlight and gutter caret (#324)

### Bug Fixes

- *(event_source)* Correct 4 bugs in kitty keyboard protocol parser (#330)
- Preserve revision diffs in git mode (viewing_revision guard) (#326)
## [0.9.0] - 2026-06-26

### Bug Fixes

- Match keybindings by physical key position via kitty protocol (#312)
- Line-blame popup closes on any navigation / background reload (#316)
- Y/Y copy path for selected directories in tree (#317)
- Tree mouse-scroll no longer snaps back to selection on watcher refresh (#319)
- Replace unwrap() with let-else early returns in popup draw paths (panic-safety) (#322)
- Remove alt-modified keybindings from defaults (#323)
## [0.8.0] - 2026-06-24

### Features

- *(config)* Two-layer config — sparse user file, refreshed default reference (#242)
- Add match highlighting, selection tracking, and mouse fix to inline tree filter (#243)
- Enforce tests-with-code via pre-commit hook and CI gate (WS-1) (#245)
- *(plugins)* Auto-register bundled plugins, install-on-toggle, show kind in palette (#244)
- Content pane active-line highlight + blame the selected line (#198) (#252)
- Go to line dialog (#253)
- Status bar line:col position + language indicator (#255)
- Plugin manifest format (plugin.toml) + discovery (#259)
- Sort tree by extension then name (dirs still first) (#261)
- Persist session state across restarts (#258)
- Git-backed plugin registry (index.json) + cache (#221) (#263)
- WS-5 LanguageProvider abstraction — uniform highlight+fold, LSP-ready (#262)
- Double-click directory in tree to change root (#270) (#273)
- *(plugin)* Send git info on init, add tests, update docs (#215) (#275)
- Per-file-type code folding via the plugin system (#201) (#280)
- Bump plugin protocol to v2 (WS-6) (#285)

### Bug Fixes

- Status bar shrinks gracefully on narrow terminals (#257)
- *(plugin)* Remove stale git-diff/git-log from bundled plugins (#264)
- Surface syntax plugins in plugin palette and gate on enabled state (#266) (#274)
- Set icons_enabled when iconize plugin sends set_icon_map (#277)
- Persist plugin enable/disable state across restarts (#278)
- Mouse wheel over tree scrolls viewport, not cursor (#276)
- Eliminate markdown rendering desync between plugin and core (#279)
- *(plugin)* Structural per-plugin state teardown on disable/crash (#287)

### Performance

- Eliminate wall-clock waits in tests, add nextest parallel config + slow group (#234) (#246)
## [0.7.0] - 2026-06-21

### Features

- Add subprocess-based plugin system (#88) (#181)
- Add breadcrumb path bar to tree panel (#69) (#182)
- Detect file encoding and line endings in statusbar (#189)
- Bundle git diff and file log as bundled plugins (#183) (#188)
- Recent files quick switcher (Ctrl+O) (#191)
- Copy file path to clipboard (#97) (#192)
- Indentation guides in tree pane (#195)
- Plugin manager overlay (runtime enable/disable) (#194)
- Add collapse-all / expand-all tree actions (closes #156) (#196)
- Terraform syntax highlighting via plugin system (#151) (#208)
- Plugin set_content action with ANSI SGR support (#186) (#217)
- Plugin protocol on_theme_change + set_content, ANSI parser, extract markdown to bundled plugin (#227)
- Cycle diff modes (unstaged/staged/all) with S keybinding (#231)
- File-type icons in the tree via iconize plugin (#197) (#225)

### Performance

- Skip SyntaxSet builder round-trip when no extra syntaxes (#228)
## [0.6.0] - 2026-06-15

### Features

- *(watch)* Add opt-in auto-reload on file change (closes #94) (#165)
- *(mouse)* Drag tree/content splitter to resize tree pane (#170)
- Add line-number gutter toggle (config + keybinding) (#173)

### Bug Fixes

- *(mouse)* Scroll tree viewport instead of cursor when tree_independent_scroll is on (#164)

### Performance

- Load files, highlighting, and git diffs off the main thread (#163)
## [0.5.5] - 2026-06-15

### Bug Fixes

- *(input)* Ignore key Release events so keys aren't handled twice on Windows (#146)
- *(config)* Add tree-viewer/ subdir to Linux/macOS global config path (#148)

### Performance

- *(reload)* Drive tree/git refresh from filesystem events, not a blind timer (#147)
- *(tree)* Slice nodes to viewport before building ListItems (#149)
- *(search)* Pre-split content cache lines to eliminate per-keystroke allocations (#150)
## [0.5.0] - 2026-06-14

### Features

- *(diff)* Add side-by-side diff view with word-level highlighting (#129)
- YAML validation, anchor/alias counting, and statusbar indicators (#65) (#130)
- Add light theme presets (vscode-light, catppuccin-latte, solarized-light, pink) (#133)
- *(tree)* Add independent tree scroll config option (#131)
- *(config)* Validate config schema with unknown-key suggestions (#132)
- *(statusbar)* Colorful per-segment color semantics (#135)
- Add first-class Windows support (#139)

### Bug Fixes

- Guard visual-line key handling behind focus == Content (#138)
## [0.4.0] - 2026-06-14

### Features

- Inline git blame annotations in content pane (#110)
- JSON syntax highlighting and pretty-print toggle (#106)
- CLI --help/--version, status bar version, and About popup (#119)
- Add search_context_lines config for showing N lines after a content match (#120)
- Add keep_search_query config option to persist last search (#121)
- Seed default config and themes on first run, add palette action to open config in editor (#124)
- YAML code folding with gutter markers and commands (#125)

### Bug Fixes

- *(ci)* Use deploy key for publish job push to main (#127)
## [0.2.9] - 2026-06-14

### Features

- Command palette (Ctrl+P) for discovering and running actions (#85)
- Home/End keys and scrollbar mouse drag (#98)
- Add git blame foundation to git.rs (#102)
## [0.2.0] - 2026-06-13

### Features

- Transient scrollbar overlay + scroll percentage in status bar (#34)
- In-file search with match highlighting and n/N navigation (#36) (#35)
- Show git branch and dirty status in statusbar (#36)
- Pin line-number gutter during horizontal scroll (#38)
- Add just new/pr commands and document agent workflow (#41)
- Add GitHub Pages docs site with mdBook (#60) (#79)
- One-line installer (no Rust toolchain required) (#84)

### Bug Fixes

- Fix formatter
- Fix formatter
- Surface walk errors in statusbar instead of silently dropping them (#53)
- Allow text selection in markdown preview when word wrap is enabled (#55)
- Set current_file only after file read succeeds (#45) (#54)
- Correct vertical position mapping for mouse selection with word wrap (#78)
- Render markdown links with accent color and underline (#43) (#77)
- Warn on malformed config instead of ignoring it silently (#80)

### Performance

- Reuse SkimMatcherV2 instead of re-allocating per keystroke (#51)
- Cache file contents and debounce content search (#44) (#76)
