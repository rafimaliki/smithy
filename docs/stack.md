Scope: the technology choices for Smithy and why. Read before adding a crate, changing the UI toolkit, or deciding where a feature's code lives.

# Tech stack

| Area | Decision | Why |
|---|---|---|
| Language | Rust, Windows 11 only | Small native binary, low RAM. No other platform until asked. |
| UI | gpui (Zed's toolkit), gated by a spike | Built for code editors; GPU text, low memory. |
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

Under 150 MB RAM idle and under 1 s cold start, measured on a release build with **core only** and one folder open. Add-ons that start processes (language servers, terminal) are measured separately and are not part of this number. Checked in the definition of done.

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

gpui is not a git-only dependency any more: `gpui = "0.2.2"` is on crates.io. The empty gpui window alone uses 132 of the 150 MB budget, so the file tree, buffers and tree-sitter would have to fit in about 18 MB. The 1 s cold-start target holds for both. Window-handle time is a rough proxy for first paint. The decision between gpui and egui is open.

## Rejected

- Webview or Electron (Tauri included): works against the low-RAM goal.
- `gix`: diff and merge-base features less complete than `git2` today.
- Bundling language servers, autocomplete, diagnostics, rename: not in v1.
- A third-party plugin API: far larger than the app it extends. Revisit only if add-ons prove insufficient.
