# cardthing

A simple, fast command-line tool for managing task cards, written in Rust. A spiritual successor to [cardigan](https://github.com/markryall/cardigan).

## Features

- **Three interfaces**: CLI commands, interactive shell, or live web board
- **Simple storage**: Cards stored as human-readable TOML files in `.cards/` directory
- **Git-friendly**: Each card is a separate file, perfect for version control
- **Rich metadata**: Name, status, owner, description, tags, and timestamps
- **Filtering**: List cards by status, owner, or tag
- **Multiple output formats**: Table view or JSON export
- **Colored output**: Visual status indicators (Todo=Yellow, InProgress=Blue, Done=Green, Blocked=Red)
- **Web board**: Kanban view with drag-and-drop, live reload, keyboard navigation, and column management
- **Checklists**: Per-card checklist items with progress tracking, manageable from CLI or web
- **Due dates**: Optional due date per card (`--due YYYY-MM-DD`)
- **Priority**: Optional priority field (high, medium, low) with color coding
- **Search**: Full-text search across card names, descriptions, and tags
- **Stats**: Summary counts by status, priority, and owner

## Installation

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs/))

### Build from Source

```bash
# Clone the repository
git clone https://github.com/markryall/cardthing.git
cd cardthing

# Build release version
cargo build --release

# The binary will be at target/release/cardthing
# Optionally, install it to your PATH
cargo install --path .
```

## Usage

cardthing supports three modes of operation:

### CLI Mode (One-shot commands)

Perfect for scripting and quick operations:

```bash
# Add a new card
cardthing add "Fix login bug" --description "Users can't log in" --status todo

# Add a card with owner, tags, due date, and priority
cardthing add "Implement feature X" \
  --description "Add new feature" \
  --status inprogress \
  --owner Alice \
  --tags feature,high-priority \
  --due 2026-07-15 \
  --priority high

# Edit a card
cardthing edit "Fix login bug" --status inprogress --owner Bob

# Set a due date and priority
cardthing edit "Fix login bug" --due 2026-07-01 --priority medium

# Clear a due date
cardthing edit "Fix login bug" --clear-due

# List all cards
cardthing list

# Filter by status
cardthing list --status todo

# Filter by owner
cardthing list --owner Alice

# Filter by tag
cardthing list --tag high-priority

# Output as JSON
cardthing list --format json

# Show full details of a card (description, checklist, due date, priority)
cardthing show "Fix login bug"

# Search cards by name, description, or tag
cardthing find "login"
cardthing search "auth"   # alias

# Show stats (counts by status, priority, owner)
cardthing stats

# Delete a card
cardthing rm "Fix login bug"
cardthing delete "Fix login bug"  # alias

# Manage checklist items
cardthing checklist "Fix login bug" add "Reproduce the issue"
cardthing checklist "Fix login bug" add "Write a failing test"
cardthing checklist "Fix login bug" toggle 1   # toggle item 1 checked/unchecked
cardthing checklist "Fix login bug" remove 2   # remove item 2 (1-based index)
```

### Shell Mode (Interactive REPL)

For interactive work with state preservation:

```bash
# Enter interactive mode
cardthing
# or explicitly
cardthing shell
```

Once in shell mode:

```
cardthing> help
Available commands:
  touch  <name> [description]     Create a new card
  ls     [--status <s>] [--owner <o>]  List all cards
  cd     <card>                    Enter card context
  set    <field> <value>           Set a field (in card context)
  edit   [card] <field> <value>    Edit a card field
  rm     <card>                    Delete a card
  help                             Show this help
  exit                             Exit shell

# Create cards
cardthing> touch "Write docs" Update the README with examples

# List cards
cardthing> ls

# Enter a card's context (like cd in a shell)
cardthing> cd "Write docs"
Entered card context: Write docs
  Status: todo
  Owner: -
  Description: Update the README with examples

# Update fields in the current card context
cardthing(Write docs)> set status inprogress
cardthing(Write docs)> set owner Alice

# Leave card context
cardthing(Write docs)> cd
Left card context

# Edit a card directly
cardthing> edit "Write docs" status done

# Delete a card
cardthing> rm "Write docs"

# Exit shell
cardthing> exit
```

### Web Board Mode

Launches a local web server with a live kanban board:

```bash
cardthing serve
# or on a custom port
cardthing serve --port 8080
```

Open `http://localhost:3000` in your browser to see a kanban board organized by status. The board updates automatically when cards change on disk (e.g. from CLI commands or git operations), so it works as a live dashboard alongside other workflows.

**Board features:**

- **Drag and drop** cards between columns to change status
- **Click any card** to open an edit modal (description, status, owner, tags, checklist)
- **New Card button** to create cards directly from the browser
- **Checklist progress bar** shown on each card
- **Column management** — add, rename, reorder (drag the column header), and delete columns (only when empty) via the `+ Column` button and per-column controls

**Keyboard shortcuts:**

| Key | Action |
|-----|--------|
| `n` | New card |
| `j` / `k` | Move focus down / up |
| `h` / `l` | Move focus left / right |
| `[` / `]` | Move focused card left / right (changes status) |
| `Enter` | Edit focused card |
| `Esc` | Deselect / close |
| `?` | Toggle shortcuts panel |

Inside the edit modal, checklist items support `Enter` to add a new item below, `Backspace` on an empty item to delete it, `Ctrl+Space` to toggle checked, and arrow keys to move between items.

### Worker Mode (Agent Automation)

`cardthing work <profile>` runs a long-lived loop that polls a column for unowned cards, hands each one to a coding agent (e.g. `claude`), and moves it forward when the agent finishes:

```bash
cardthing work implement
# process at most 3 cards then exit
cardthing work implement --max-cards 3
```

Worker behaviour is defined by `[[workers]]` profiles in `.cards.toml`:

```toml
[[workers]]
name = "implement"          # profile name passed to `cardthing work <name>`
watch = "todo"              # status to poll for unowned, non-flagged cards
done = "done"                # status the agent should set when finished
prompt = "Implement the card."       # inline system prompt ...
prompt_file = ".cards/prompts/implement.md"  # ... or a file (exactly one of the two)
model = "sonnet"             # optional: passed as --model to the agent
effort = "medium"             # optional: passed as --effort to the agent
allowed_tools = [             # optional: passed as --allowed-tools (defaults to Bash(cardthing:*))
  "Read", "Glob", "Grep", "Edit", "Write",
  "Bash(cargo:*)", "Bash(cardthing:*)",
]
```

**Claiming:** a worker finds the oldest unowned card in `watch` status, then claims it by setting the card's `owner` field to a randomly generated, cute worker name (e.g. `zesty-flamingo-62`) before invoking the agent. The owner field doubles as a lock — it's how workers avoid stepping on each other and how the board shows a card is being actively worked. The owner is **always cleared** once the agent run finishes, whether it succeeded, failed, or needs a human, so the card becomes free again as soon as it's unflagged.

**Watching the run:** while a card is claimed, the web board marks it with a pulsing `agent` badge (from the card's `agent` flag). Every run is logged to `.cards/.logs/<card>-<timestamp>.md` with the agent's full stdout/stderr, so you can review what it did after the fact.

**Clarification round-trip:** if the agent can't complete a card without a human decision, it runs `cardthing edit "<card>" --needs-human`, appends its questions to the card's description, and leaves the card in its current column. Flagged cards show a `🙋` needs-human indicator on the board and are skipped by every worker's polling loop. A human answers the questions directly in the description, then clears the flag — either via the checkbox in the web edit modal or with `cardthing edit "<card>" --clear-needs-human` — which makes the card eligible for pickup again. If an agent run ends without the card being marked done or flagged needs-human, the worker flags it needs-human itself so nothing silently falls through the cracks.

## Card Data Model

Each card contains:

- **name**: Unique identifier for the card
- **status**: One of `todo`, `inprogress`, `done`, `blocked` (configurable via `.cards.toml`)
- **owner**: Optional assignee (can be any string)
- **description**: Card details
- **created_at**: Timestamp when card was created
- **updated_at**: Timestamp of last modification
- **tags**: Array of tags for categorization
- **priority**: Optional priority — `high`, `medium`, or `low`
- **due_at**: Optional due date (ISO 8601)
- **checklist**: Optional list of checklist items, each with text and checked state

## Storage

Cards are stored in a `.cards/` directory in your current working directory. Each card is a separate TOML file:

```toml
# .cards/fix-login-bug.toml
name = "Fix login bug"
status = "inprogress"
owner = "Bob"
description = "Users can't log in"
priority = "high"
due_at = "2026-07-01T00:00:00Z"
created_at = "2025-12-23T02:26:43.056620Z"
updated_at = "2025-12-23T02:27:03.457657Z"
tags = ["bug", "critical"]

[[checklist]]
text = "Reproduce the issue"
checked = true

[[checklist]]
text = "Write a failing test"
checked = false
```

Files are named using a sanitized version of the card name (lowercase, spaces to hyphens, special characters removed).

## Examples

### Typical Workflow (CLI Mode)

```bash
# Create a new feature card
cardthing add "User authentication" \
  --description "Implement OAuth2 login" \
  --status todo \
  --tags auth,security

# Start working on it
cardthing edit "User authentication" \
  --status inprogress \
  --owner Alice

# See what you're working on
cardthing list --status inprogress --owner Alice

# Mark it as done
cardthing edit "User authentication" --status done

# Review all completed work
cardthing list --status done
```

### Typical Workflow (Shell Mode)

```bash
$ cardthing
cardthing> touch "Code review" Review pull request #42
cardthing> cd "Code review"
cardthing(Code review)> set owner Bob
cardthing(Code review)> set status inprogress
cardthing(Code review)> cd
cardthing> ls --status inprogress
cardthing> exit
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration_tests

# Run with output
cargo test -- --nocapture
```

### Project Structure

```
cardthing/
├── src/
│   ├── main.rs           # Entry point
│   ├── lib.rs            # Library exports
│   ├── cli.rs            # CLI argument parsing
│   ├── shell.rs          # Interactive REPL
│   ├── error.rs          # Error types
│   ├── commands/         # Command implementations
│   │   ├── init.rs
│   │   ├── serve.rs      # Web board (axum + SSE + SortableJS)
│   │   └── mod.rs
│   ├── models/           # Data models
│   │   └── card.rs
│   └── storage/          # File I/O
│       └── cards.rs
└── tests/
    └── integration_tests.rs
```

## License

MIT

## Credits

Inspired by the original [cardigan](https://github.com/markryall/cardigan) Ruby gem.
