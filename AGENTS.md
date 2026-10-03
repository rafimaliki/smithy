# Smithy

A lightweight, native Windows code editor: file-tree sidebar, editor, and a git/GitHub review view (working changes, branch-vs-branch diffs, pull requests viewed locally) for people who work with coding agents.

Status: **v1 built** (core plus all nine add-ons, merged through PRs #4-#24). Stack and features are in `docs/`, screens and the build plan are on the framery board (page `flows`, page `plan`). Measured: about 110 MB working set idle with a folder open and no add-on on, window in under a second (see `docs/stack.md`). Not done: an installer, light-weight tuning of the 48 MB exe, and the hand checks listed under "Known gaps" in `docs/tasks.md`.

```
npx framery          # design board (screens and flows)
cargo test           # once code exists
cargo clippy -- -D warnings
```

## Hard constraints

- **Small and low-RAM** — the product's reason to exist is being lighter than VS Code. No Electron, no bundled browser engine unless the stack decision records a measured exception.
- **Windows first** — it must run on Windows 11 without extra runtimes. Other platforms are not a goal yet; do not pay for portability that is not asked for.
- **Small core, everything else an add-on** — the core is file tree, tabs and editing. Every other feature is a first-party add-on, off by default and not loaded when off. A new feature either fits the core list in `docs/features.md` or becomes an add-on; it never makes the core heavier.
- **GitHub only for now** — one provider. The user supplies their own token. Do not abstract over other providers until a second one is real.
- **Tokens never touch disk in plain text or git** — store in the OS credential store; `.env` and `*.token` are gitignored.

## Layout

```
framery/smithy/   design board: screens and flows, edited through framery tools, not by hand
docs/             stack.md and features.md; add files only when they have content
src/              Rust code by domain: editor, tree, workspace (window, tabs, chrome), addon (registry + one folder per add-on), git, github
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
| `docs/stack.md` | choosing a crate or UI toolkit, or deciding where a feature's code lives (core or add-on) |
| `docs/features.md` | deciding whether a feature belongs, and whether it is core or an add-on |
| `docs/tasks.md` | picking up a task: how an agent works here and what each remaining piece needs |
| `framery/smithy` (the board) | building any screen: read its frames and group descriptions first |

## Definition of done

- The change landed on a branch and merged by pull request; `main` was never written to directly.
- Design changes: the flow is settled on its framery canvas and `npx framery doctor` says ok.
- Code changes: `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` pass.
- Idle RAM and cold-start time of the built app (core only, one folder open) are checked against the targets in `docs/stack.md` once code exists.
- No file over the ceiling without a note here saying why.
