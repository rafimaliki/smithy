# Smithy

A lightweight, native Windows code editor: file-tree sidebar, editor, and a git/GitHub review view (working changes, branch-vs-branch diffs, pull requests viewed locally) for people who work with coding agents.

Status: **design phase**. No app code yet. Tech stack and features are settled through `/grill-me` and the framery board first; do not scaffold Rust code until the stack decision is written in `docs/`.

```
npx framery          # design board (screens and flows)
cargo test           # once code exists
cargo clippy -- -D warnings
```

## Hard constraints

- **Small and low-RAM** — the product's reason to exist is being lighter than VS Code. No Electron, no bundled browser engine unless the stack decision records a measured exception.
- **Windows first** — it must run on Windows 11 without extra runtimes. Other platforms are not a goal yet; do not pay for portability that is not asked for.
- **Only necessary features** — each feature must serve browsing files, editing them, or reviewing git/GitHub changes. Anything else (extensions, debugger, terminal, etc.) needs an explicit decision in `docs/`.
- **GitHub only for now** — one provider. The user supplies their own token. Do not abstract over other providers until a second one is real.
- **Tokens never touch disk in plain text or git** — store in the OS credential store; `.env` and `*.token` are gitignored.

## Layout

```
framery/smithy/   design board: screens and flows, edited through framery tools, not by hand
docs/             decisions (created by /grill-me; add files only when they have content)
src/              Rust code, by domain (tree, editor, git, github), once the stack is decided
```

## Code rules

- **karpathy-guidelines** — surgical diffs, no speculative scope, state assumptions, leave a verifiable check.
- **ponytail (full)** — laziest thing that works. Mark shortcuts `// ponytail: <ceiling>, <upgrade path>`.
- **caveman (ultra)** — chat prose only. Code, comments, commits, and docs stay normal clear English.
- One responsibility per file. 300-line soft ceiling, 500 hard (tests, generated code and data excluded) — past it, split along a seam, not at a line count.
- SOLID, read as: one reason to change per module; extend by adding rather than by editing a switch; depend on the narrowest interface that does the job.
- `main` is a thin shell; logic lives in testable modules. Tests mirror the source tree.

## Design board

- The design lives in `framery/` and is edited through the framery tools, not by hand.
- **A flow is settled on its canvas before it is built.**
- Upgrading framery is a choice: install another tag, `npx framery init`, `npx framery doctor`.

## How changes land

**Nothing is written to the default branch directly.** Every change arrives as a branch and a pull request and is squash-merged: one commit per change on `main`, its subject ending in the PR number, `feat(scope): description (#N)`. The branch is deleted on merge.

- Branch `<type>/<feature-name>`; commits `<type>: <description>`, lowercase imperative, no attribution trailer.
- One change, one branch. A follow-up joins the open PR's branch; an unrelated change starts its own.
- A PR body carries **what**, **why** and a **test plan** that says what was run and observed, and what could not be run.
- Merge only with the person's go-ahead. Squash only, never a force push, never leave `main` red.

## Skills

`/feat` for feature work, `/git` for every git/gh operation. Conventions here win over their defaults.

## Docs

| File | Read when |
|---|---|
| `docs/` (empty until `/grill-me` finishes) | choosing a crate, a UI toolkit, or deciding whether a feature belongs |

## Definition of done

- The change landed on a branch and merged by pull request; `main` was never written to directly.
- Design changes: the flow is settled on its framery canvas and `npx framery doctor` says ok.
- Code changes: `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` pass.
- Idle RAM and cold-start time of the built app are checked against the targets in `docs/` once they exist.
- No file over the ceiling without a note here saying why.
