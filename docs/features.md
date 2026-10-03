Scope: what Smithy v1 does and does not do. Read before starting a feature or deciding whether an idea belongs.

# v1 features

1. One folder per window, file tabs, no split panes.
2. Sidebar with a nested, indented file tree.
3. Edit-light editor: open, edit, save, undo/redo, find, syntax highlighting, reload when a file changes on disk. No LSP, no autocomplete.
4. Source-tree panel with working-tree status.
5. Diff view with red/green lines and word-level highlights, toggling between unified and side-by-side.
6. Stage/unstage by file and hunk, discard, commit with a message box.
7. Compare the current branch with another branch, in the same diff view.
8. GitHub pull requests: list, metadata (title, author, status), fetch the PR ref, diff against the base locally (merge-base, to match GitHub's "Files changed"). No review comments.
9. Settings screen to register the GitHub token.

# Out of v1

Embedded terminal, agent-specific features, PR comments, branch switching, push/pull, split panes, multi-root workspaces, LSP, non-GitHub providers.

# Deferred questions

- Installer story: portable `.exe` is assumed until asked.
- License.
