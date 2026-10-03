Scope: the technology choices for Smithy and why. Read before adding a crate, changing the UI toolkit, or deciding where a feature's code lives.

# Tech stack

| Area | Decision | Why |
|---|---|---|
| Language | Rust, Windows 11 only | Small native binary, low RAM. No other platform until asked. |
| UI | gpui 0.2.2 (Zed's toolkit, on crates.io); egui is the fallback | Built for code editors; GPU text. An empty window is 132 MB, so the RAM target is 200 MB (see Targets). |
| Git | `git2` (libgit2) | Mature diff, status, blame, merge-base; no git install needed. Needs MSVC build tools. |
| GitHub | REST API for PR list and metadata; `git2` fetches `refs/pull/N/head` | The diff is computed locally, same code path as branch compare. |
| Auth | Personal access token in Windows Credential Manager (`keyring`) | Simplest; no OAuth app to register. Never on disk or in git. |
| Syntax highlighting and symbol outline | `tree-sitter`, one grammar per language, loaded when a file of that type opens | Incremental and fast. The same parse feeds symbol search, so symbols need no language server. |
| File watching | `notify` | Edits made by other tools reload without a manual refresh. |
| Buffer | `ropey` or gpui's own, decided in the spike | |

## Core and add-ons

Smithy has a small core that is always on. Everything else is a **first-party add-on**: compiled into the one binary, off by default, switched on in Settings. An off add-on is never initialized, so it costs no memory. There is no third-party plugin API; each add-on is its own module with a small interface to the core, so one could be added later.

- **Core:** open folder, file tree, tabs (pin, reopen, unsaved prompt), editing with undo, find in file, word wrap, syntax highlighting, themes, shortcuts, reload on external change, large and binary file guard.
- **Add-ons:** Source control, Pull requests (needs Source control), Search, Language servers, Terminal, Split panes, Image and SVG viewer, PDF viewer, Markdown preview.

An add-on's rail button, menus and settings are hidden when it is off. Turning one on that depends on another asks to turn on both.

| Add-on | Implementation | Weight |
|---|---|---|
| Source control | `git2`: status, diff, blame, stage hunks, commit | Light |
| Pull requests | GitHub REST plus local fetch of the PR ref | Light |
| Search | text and file search; symbols from the tree-sitter outline | Light |
| Language servers | LSP client using servers the user already has (rust-analyzer, typescript-language-server, ...). Definition, references, back/forward only. Not bundled, off per language, started on first use, stopped when idle | Heavy: a separate process per language |
| Terminal | ConPTY plus a terminal emulator crate; PowerShell, cmd or Git Bash. Sessions are created only when opened | Heavy: a separate process per session |
| Split panes | two editor groups | Light |
| Image and SVG viewer | image decoding crate; SVG through the toolkit's renderer | Light |
| PDF viewer | the Windows built-in PDF component, nothing bundled | Medium |
| Markdown preview | `pulldown-cmark` | Light |

Crate names above are proposals; the spike and the first add-on settle them.

## Targets

Under 200 MB RAM idle and under 1 s cold start, measured on a release build with **core only** and one folder open. Add-ons that start processes (language servers, terminal) are measured separately and are not part of this number. Checked in the definition of done. The RAM target was 150 MB until the spike showed an empty gpui window already uses 132 MB; it was loosened to 200 MB to keep gpui's text rendering. Still far below VS Code, which is why the product exists.

## Keeping it light

- Draw only what is on screen: the file tree, diffs and the editor are virtualized.
- Large files (limit set in the spike) open read-only without highlighting; binary files are never loaded.
- Grammars, language servers and terminal sessions load on first use.
- Off add-ons add binary size, not RAM. Downloadable add-ons would remove even that, and are a later step.

## gpui spike (first code written)

Risk: gpui is a git-only dependency with newer Windows support and API churn. Before any feature work, build a throwaway app that opens a window, scrolls a 50k-line file, renders a tree, accepts text input, and records RAM. If it fails, fall back to egui and update this file.

### Result so far (2026-10-03, Windows 11, release build, empty window with a 50k-line virtual list)

Throwaway code is in `spike/gpui` and `spike/egui`. Not yet tested: tree, text input, scrolling under load.

| | gpui 0.2.2 (crates.io) | egui/eframe 0.31 |
|---|---|---|
| Builds on Windows 11 with MSVC Build Tools | yes, first try, 4m32s cold | yes, 1m10s cold |
| Exe size | 10.9 MB | 4.9 MB |
| Window handle appears (warm, 3 runs) | 557-674 ms | 188-189 ms |
| First run after build | 1.3 s | 0.5 s |
| Working set, idle | 132 MB (194 MB on the very first run) | 83-84 MB |
| Private bytes, idle | 115 MB | 66 MB |
| Renders and scrolls | yes (checked by screenshot) | not checked by screenshot |

gpui is not a git-only dependency any more: `gpui = "0.2.2"` is on crates.io. The empty gpui window alone uses 132 MB, which is why the RAM target moved from 150 to 200 MB and gpui was kept. The 1 s cold-start target holds for both. Window-handle time is a rough proxy for first paint. Still to check before building features: RAM while typing and scrolling. If gpui passes 200 MB under load, reopen the egui decision.

### Core build under real use (scaffold PR, 2026-10-03, release build, this repo open)

Own editor (ropey buffer, virtualized lines, typing, save, undo, unsaved prompt), file tree, tabs, no add-ons on: working set 108 MB right after open, 83 MB with a file open and after typing; private bytes 65-70 MB. Window handle in 0.58-0.66 s, measured through a PowerShell poll that adds its own overhead, so the real figure is lower. Both targets hold with room for tree-sitter, the watcher and the Search/Source control add-ons. The load check in the spike section is closed: gpui stays.

### Final v1 build (all nine add-ons compiled in, none on, clean settings)

Release exe 48 MB. Folder open: 107 MB working set / 78 MB private; 112 MB / 81 MB with a Rust file open and highlighted. Window handle in 0.94 s including the PowerShell poll that measures it. Both targets hold. Add-ons that start processes (terminal, language servers) are not part of this number, as agreed above. The exe grew from 12 MB to 48 MB because every add-on is compiled in; off add-ons cost disk, not RAM.

## Rejected

- Webview or Electron (Tauri included): works against the low-RAM goal.
- `gix`: diff and merge-base features less complete than `git2` today.
- Bundling language servers, autocomplete, diagnostics, rename: not in v1.
- A third-party plugin API: far larger than the app it extends. Revisit only if add-ons prove insufficient.
