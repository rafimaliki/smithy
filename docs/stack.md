Scope: the technology choices for Smithy and why. Read before adding a crate, changing the UI toolkit, or questioning a platform decision.

# Tech stack

| Area | Decision | Why |
|---|---|---|
| Language | Rust, Windows 11 only | Small native binary, low RAM. No other platform until asked. |
| UI | gpui (Zed's toolkit), gated by a spike | Built for code editors; GPU text, low memory. |
| Git | `git2` (libgit2) | Mature diff, status, merge-base; no git install needed. Needs MSVC build tools. |
| GitHub | REST API for PR list and metadata; `git2` fetches `refs/pull/N/head` | The diff is computed locally, same code path as branch compare. |
| Auth | Personal access token, stored in Windows Credential Manager (`keyring`) | Simplest; no OAuth app to register. Never on disk or in git. |
| Syntax highlighting | `tree-sitter` | Incremental and fast. |
| File watching | `notify` | Edits made on disk by other tools appear without a manual refresh. |
| Buffer | `ropey` or gpui's own, decided in the spike | |

## Targets

Under 150 MB RAM idle and under 1 s cold start, measured on a release build and checked in the definition of done.

## gpui spike (first code written)

Risk: gpui is a git-only dependency with newer Windows support and API churn. Before any feature work, build a throwaway app that opens a window, scrolls a 50k-line file, renders a tree, accepts text input, and records RAM. If it fails, fall back to egui and update this file.

## Rejected

- Webview or Electron (Tauri included): works against the low-RAM goal.
- `gix`: diff and merge-base features less complete than `git2` today.
- Full LSP: scope and RAM cost not justified for v1.
