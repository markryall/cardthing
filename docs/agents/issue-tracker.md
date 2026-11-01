# Issue tracker: cardthing

Issues for this repo are cardthing cards: TOML files in `.cards/`, managed with
the `cardthing` CLI. A card is addressed by its name, which is also its title.
Run `cardthing <command> --help` for the full flags.

## Conventions

- One card per issue. The card name is the title; it cannot contain `/` or `\`.
- The body is the card description. To add to it, use
  `cardthing append "<card>" "<text>" [--section "<heading>"]`: it inserts at
  the end of that section (creating `## <heading>` if missing), or at the end of
  the description, under a lock, so concurrent sessions never lose each other's
  text. `<text>` may start with `- `; pass `-` to read multi-line text from
  stdin. `cardthing edit --description` replaces the whole description; don't
  use it to add text.
- Comments and answers go in the description under a heading
  (`--section Comments`, `--section Answer`). There is no separate comment thread.
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
- **Claim**: `cardthing edit "<ticket>" --claim <driving dev>`, the session's
  first write. It is atomic and fails if someone else already owns the ticket;
  if it fails, another session got there first, so re-run the frontier query
  and take the next ticket.
- **Resolve**: `cardthing append "<ticket>" - --section Answer` with the answer
  on stdin, then `cardthing edit "<ticket>" --status done`, then
  `cardthing append "<map>" "- <ticket name>: <gist>" --section "Decisions so far"`.
  Moving the ticket to `done` unblocks the tickets that wait on it.
- **Out of scope**: move the ticket to `done`,
  `cardthing append "<ticket>" "<reason>" --section "Out of scope"`, and
  `cardthing append "<map>" "- <ticket name>: <reason>" --section "Out of scope"`
  (not Decisions so far).
