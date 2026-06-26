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
- **Web board**: Kanban view with drag-and-drop, live reload, and keyboard navigation
- **Checklists**: Per-card checklist items with progress tracking

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

# Add a card with owner and tags
cardthing add "Implement feature X" \
  --description "Add new feature" \
  --status inprogress \
  --owner Alice \
  --tags feature,high-priority

# Edit a card
cardthing edit "Fix login bug" --status inprogress --owner Bob

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

## Card Data Model

Each card contains:

- **name**: Unique identifier for the card
- **status**: One of `todo`, `inprogress`, `done`, `blocked`
- **owner**: Optional assignee (can be any string)
- **description**: Card details
- **created_at**: Timestamp when card was created
- **updated_at**: Timestamp of last modification
- **tags**: Array of tags for categorization

## Storage

Cards are stored in a `.cards/` directory in your current working directory. Each card is a separate TOML file:

```toml
# .cards/fix-login-bug.toml
name = "Fix login bug"
status = "inprogress"
owner = "Bob"
description = "Users can't log in"
created_at = "2025-12-23T02:26:43.056620Z"
updated_at = "2025-12-23T02:27:03.457657Z"
tags = ["bug", "critical"]
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
