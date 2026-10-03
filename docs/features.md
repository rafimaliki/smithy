Scope: what Smithy v1 does, which part is core and which is an add-on, and what is out. Read before starting a feature or deciding whether an idea belongs. The screens for all of this are on the framery board (`framery/smithy`, page `flows`).

# Core (always on)

1. One folder per window; a launch screen with Open folder and recent projects.
2. Sidebar with a nested, indented file tree; file icons by extension (brand icons for an app-defined list, or simple icons, a setting).
3. File menu: open, copy, cut, paste, rename, delete (to the Recycle Bin), copy path, reveal in File Explorer.
4. Tabs: pin, close others or to the right, reopen closed tab, unsaved-changes prompt.
5. Editor: edit, save, undo/redo, find in file, word wrap, syntax highlighting chosen by file extension (24 languages), a language picker per file, reload on external change (silent, or a bar when there are unsaved edits).
6. Large files open read-only; binary files are not loaded.
7. Settings: themes (Nord Dark default, taken from the owner's VS Code, plus Warm graphite, Midnight indigo, Daylight), editor font size, keyboard shortcuts (rebindable, conflicts are named), add-ons.

# Add-ons (off by default)

| Add-on | What it adds |
|---|---|
| Source control | Changes view with staged and unstaged files; inline or side by side diff; stage, unstage and discard by file or hunk (discard asks first); commit; compare the current branch with another; git blame (current line, or a gutter with a commit card) |
| Pull requests | PR list with a checks summary, PR detail, Review changes fetches the PR ref and diffs it locally (read-only, no comments). Settings: GitHub token. Needs Source control |
| Search | Sidebar search by text, file or symbol; Search everywhere (Ctrl+Shift+O) with All / Files / Symbols / Text; Find in folder |
| Language servers | Ctrl+Click go to definition, references peek, back and forward, per language, using servers the user has installed |
| Terminal | Bottom panel, PowerShell / cmd / Git Bash, several sessions, Open in Terminal |
| Split panes | Two editor groups side by side |
| Image and SVG viewer | PNG, JPG, GIF, WebP, BMP, SVG (Preview / Code) |
| PDF viewer | Page scroll, zoom, fit width |
| Markdown preview | Code / Side by side / Preview |

Empty and error states are designed: not a git repository, clean tree, no GitHub token, GitHub unreachable.

# Out of v1

Autocomplete, diagnostics, rename or any editing intelligence from language servers; bundled servers; PR comments and reviews; branch switching, push, pull; multi-root workspaces; more than two editor groups; a command palette; third-party extensions or a plugin API; non-GitHub providers; other platforms.

# Deferred questions

- Installer story: a portable `.exe` is assumed until asked.
- License.
- Default large-file limit and image / PDF size limits, set in the spike.
- Whether add-ons should become separately downloadable.
