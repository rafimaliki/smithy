---
name: framery
description: >
  Read and edit a project's design in framery, the local design studio: flows of frames (html screens of
  any size), groups, flowchart nodes, tables and labelled arrows on free canvas pages, plus design
  tokens and the implementation plan (a table). Use when asked to design, mock up, review, lay out, connect, describe or
  change a screen, flow, state, token or plan item in this project, or to learn what a flow does before
  building it, or to undo, revert or go back on a change to the design (every change is recorded and restorable).
  Works through the framery MCP tools, or `npx framery cmd <tool> '<json>'` without MCP.
---

# Framery

A studio of pages. Each page is a free canvas of **frames** (an html file drawn at any size), **groups**
(a darkened container, like a Figma section), **nodes** (flowchart shapes) and **arrows** (a labelled,
toned line between two items, or between two elements inside frames). People look at it in the browser;
you change it through tools. Dragging in the UI is not a thing yet: placement, grouping and arrows all
come from you.

## Read first

1. `get_project` — the pages and how big each is.
2. `outline {page}` — a page as text in reading order: groups with their descriptions, each frame with
   its size, description, `src` html and where its arrows lead (`-> next "label" [tone]`). This is the
   cheapest way to know what a flow is for and what its rules are. Prefer it to opening images.
3. `get_page {page}` — the same data as JSON when you need exact positions.
4. A frame's picture: `<project>/.cache/frames/<id>.webp`. Its source: the `src` html file.
5. A page can hold a **table** (a plan, a matrix). `outline` prints it as a grid, one line per row, so
   "which flows are built?" is `outline {page: "plan"}`.
6. `list_tokens` — the design values (`--paper`, `--ink`, …) the frames use.

Descriptions are the product intent, written for whoever builds the screen. Read the group description
for the flow's rules and each frame's description for that state. Build to them; if a frame and its
description disagree, say so rather than guessing.

## Edit

All writes go through the tools. They validate, keep ids stable, and the open studio updates live.

| Want | Tool |
|---|---|
| a screen on the canvas | `add_item {type:"frame", src:"flows/x/y.html", device:"phone"|"tablet"|"desktop"|custom with w,h, title, step, description, parent}` |
| a container | `group_items {ids, title, description}` (sized to its members) |
| a flowchart shape | `add_item {type:"node", shape:"diamond"|"process"|"terminal", title}` |
| a connection | `connect {from, to, label, tone:"positive"|"negative"|"neutral"}` |
| anchor to an element | `connect {from:"frame-id#element-id", ...}` — the element needs `id="..."` or `data-anchor="..."` in the frame's html; `list_anchors {frame}` shows what exists |
| place things | `move_items {ids, dx, dy}`, `arrange {ids, direction:"row"|"column", gap}`, or give `x`,`y` on add |
| change anything | `update_item {id, patch}`, `update_arrow {id, patch}`, `remove_item`, `remove_arrow` |
| a new page | `add_page {id, title}` |
| tokens | `set_token {name, value}` |
| a table | `add_item {type:"table", columns:[{id,title,note?}], rows:[{id,title,link?:"page/item",cells:{columnId:text}}], marks:{"built":"positive"}}` |
| progress in a table | `set_cell {id, row, column, value}`: one cell, the cheapest edit; `add_row`, `remove_row`, `add_column`, `remove_column` |

Conventions that keep the board readable:

- **Arrows say what the person does.** Label with the action ("tap Save"), not the destination. `positive`
  for the happy path, `negative` for a failure or refusal, `neutral` otherwise. One colour never carries
  meaning alone: the label does.
- **An arrow starts on the control that causes it.** A screen change nearly always follows a tap, so give
  that button, field or row an `id`/`data-anchor` in the frame and `connect` from `frame#control`; the
  arrow then leaves the exact thing the person touches and ends on the screen it opens. Leave the source
  unanchored only when nothing is tapped (a timeout, a failed sign-in). Alternate states of one page
  (empty, loading, error) are not transitions: do not chain them. Connect a state to the control that
  reaches it ("tap Try again" from the error's retry button to the loading state).
- **Branches are diamonds.** A question in the diamond ("Amount entered?"), one labelled arrow per answer.
- **A group is one flow.** Frames inside it read left to right; its description holds the flow's rules.
- **Descriptions are short and specific.** What the frame is, then the rule it must keep. No restating
  what the picture shows.
- **Ids are stable and lowercase.** Do not rename an id other work refers to.
- **Frames are html files** in the project folder, built from the project's own css and tokens. A frame
  measures itself: `autoHeight: true` frames get their height from the page when previews render.

## Components

A control used in many frames should be a **component**: one definition in `library/<id>.html`
(registered in `library.json`) with `{{prop}}` placeholders, used by reference. In a frame an instance is a
marked region of plain html: `<!-- fr:component button variant="primary" label="Save" -->…<!-- /fr:component -->`.
The html between the markers is generated, so never edit it by hand: change the definition (or the instance's
props) and every frame follows.

| Want | Tool |
|---|---|
| see the library, uses and the frames using each | `list_components`, `component_usage {id}` |
| a new component | `create_component {id, html, props}`; props are `text`, `html`, `enum` (`values`: key → classes), `flag` (`output`) |
| change it everywhere | `update_component {id, html?, props?}`: every instance is rewritten |
| use it in a frame | `use_component {file, component, props, replace:"<exact html>"}` (or `before:`) |
| make repeated markup a component | `find_candidates` lists repeats; `promote_component {id, tag, block, variants, flags}` writes the definition and converts the copies it can draw exactly, reporting the rest |
| convert what is left | `convert_copies {id}` after the definition learned a prop |
| back to plain markup | `detach_component {file, component?}`, `remove_component {id}` |
| a page of all its variants | `component_sheet {id}`, then `add_item` the file as a frame |
| a better name | `rename_component {id, to, title?}`: the registry, the template and every instance. Name components for what they are (a bottom navigation bar is not a tab control) |

Props are the only thing an instance may change (`anchor`, `id` and `style` are always allowed: they become
`data-anchor`, `id` and `style` on the root). If a frame truly differs, detach that instance. Not a component yet
means plain markup: leave it, it is listed by `find_candidates`. A template may test an enum with
`{{active==home?is-active}}` (set `search: true` on that prop so `convert_copies` can find the value).

## Export

`export_item {id, format, scale}` writes a frame, a group (with its frames and arrows), a table or a node to
`<project>/exports/` as `png`, `webp`, `pdf` or `svg`. `svg` embeds the png (a browser cannot make html into
vector paths); `pdf` keeps text sharp. The studio's detail panel has the same Export button.

## History

Every project keeps an undo trail (`<project>/.history/`, never committed): each prompt's changes are one
entry, however many tool calls it took (calls less than 20 seconds apart merge), and edits made outside the
tools are caught too. The studio lists it in its History panel, where a person can go back before any entry.

| Want | Tool |
|---|---|
| what changed, newest first | `history {limit?}` |
| start a separate step | `checkpoint {label}`: call it at the start of a task, so it undoes as one thing |
| undo the last change | `undo`: one call; recorded, so calling it again redoes it |
| go back further | `history`, pick the entry just before the unwanted work, then `restore {n}` |
| go back | `restore {n}`: the state right after entry n. The current state is saved first, so a restore can be undone |

**When the person asks to undo** ("undo that", "revert", "put it back"): use these, never reverse the edit by hand
and never `git checkout` the project folder. `undo` covers "undo that"; for older work, `history` shows each
prompt as an entry (tools, files, time), so say which entry you are going back before and confirm if it is not
the latest. This works for frames, css, pages, tokens and the component library alike: it restores files, so
generated component regions and previews follow. Do not restore on your own initiative: it discards later work.

## Show your work

- `render_frames` refreshes previews, measured anchors and autoHeight heights. The studio does this on
  its own when a frame's html or css changes, so you rarely call it; call it before relying on
  `list_anchors` boxes.
- `link {page, id}` returns a URL that opens the page and focuses the item. Give it to the person
  reviewing, rather than describing where something is.

## Without MCP

`npx framery cmd <tool> '<json>'` runs any tool from a shell and prints the result;
`npx framery tools` lists them with descriptions. Start the studio with `npx framery`
(http://127.0.0.1:4173).

## Updating framery

Never update it on your own. When the person asks: `framery doctor` says what is installed and whether this project's
skill and MCP entry match; they pick a release (`npm install --save-dev github:rafimaliki/framery#<tag>`), then
`framery init` refreshes the skill and `framery doctor` confirms. Designs in `framery/` are not rewritten by an upgrade.
If a tool named here does not exist, the installed version is older than this skill: run `framery doctor`.

## Do not

- Edit `pages/*.json` by hand when a tool can do it: the tools keep arrows valid and groups fitted.
- Put design data in `.cache/`: it is generated and safe to delete.
- Invent flow behaviour. If an arrow, label or description is not in the brief, a decision doc, or the
  person's words, ask.
