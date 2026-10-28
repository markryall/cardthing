# Issue tracker: cardthing

Issues for this repo are cardthing cards: TOML files in `.cards/`, managed with
the `cardthing` CLI. A card is addressed by its name, which is also its title.
Run `cardthing <command> --help` for the full flags.

## Conventions

- One card per issue. The card name is the title; it cannot contain `/` or `\`.
- The body is the card description. `cardthing edit --description` replaces the
  whole description, so to append, `cardthing show` it first and pass the full
  new text back.
- Comments and answers go at the bottom of the description under a heading
  (`## Comments`, `## Answer`). There is no separate comment thread.
- `todo` is watched by agent workers (`cardthing work`). Never create a card
  there unless an agent should pick it up and implement it.

## When a skill says "publish to the issue tracker"

`cardthing add "<title>" --description "<body>" --tags <tags>`

## When a skill says "fetch the relevant ticket"

`cardthing show "<name>"`, or `cardthing list --format json` for every card's
raw fields.

## Wayfinding operations

Used by `/wayfinder`. The **map** is one card; its **tickets** are cards tagged
with the map's slug. They all live in the `wayfinder` column, which no worker
watches, so agent workers never pick up a decision ticket. If `.cards.toml` has
no `wayfinder` status, add one before charting:

```toml
[[statuses]]
id = "wayfinder"
label = "Wayfinder"
color = "#a855f7"
```

- **Map**: a card in the `wayfinder` column tagged `wayfinder:map`, holding the
  Destination / Notes / Decisions so far / Not yet specified / Out of scope
  body. Its **slug** is the card's filename without `.toml` (the name
  lowercased, spaces as `-`).
  `cardthing add "<map name>" --status wayfinder --tags wayfinder:map --description "<body>"`.
  Moved to `done` when the way to the destination is clear.
- **Child ticket**: a card in the `wayfinder` column tagged
  `wayfinder:<type>` (`research`/`prototype`/`grilling`/`task`) and
  `map:<slug>`, with the question as its description. No other tags.
  `cardthing add "<ticket name>" --status wayfinder --tags wayfinder:<type>,map:<slug> --description "## Question ..."`.
- **Blocking**: cardthing's native dependencies, wired in a second pass once
  every ticket exists: `cardthing edit "<ticket>" --add-blocked-by "<blocker>,<blocker>"`.
  It rejects unknown names and cycles. A ticket is unblocked when every blocker
  is `done`. Blocked tickets show a lock in `cardthing list` and on the web
  board. The `blocked` status column is unrelated; don't use it.
- **Frontier query**: `cardthing list --tag map:<slug> --status wayfinder --ready`
  lists the open, unblocked, unclaimed tickets, oldest first. First in that order
  wins.
- **Claim**: `cardthing edit "<ticket>" --owner <driving dev>`, the session's
  first write.
- **Resolve**: append the answer under `## Answer` in the ticket's description,
  then `cardthing edit "<ticket>" --status done`, then add a context pointer
  (`- <ticket name>: <gist>`) to the map card's Decisions so far. Moving the
  ticket to `done` unblocks the tickets that wait on it.
- **Out of scope**: move the ticket to `done`, add `## Out of scope` with the
  reason to its description, and list it under the map's Out of scope, not
  Decisions so far.
