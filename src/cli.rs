use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cardthing")]
#[command(about = "A simple task card manager", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new card
    Add {
        /// Name of the card
        name: String,

        /// Description of the card
        #[arg(short, long)]
        description: Option<String>,

        /// Initial status (todo, inprogress, done, blocked)
        #[arg(short, long, default_value = "todo")]
        status: String,

        /// Assign an owner
        #[arg(short, long)]
        owner: Option<String>,

        /// Add tags (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,

        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due: Option<String>,

        /// Priority (high, medium, low)
        #[arg(long)]
        priority: Option<String>,
    },

    /// Edit an existing card
    Edit {
        /// Name of the card to edit
        name: String,

        /// Update description
        #[arg(short, long)]
        description: Option<String>,

        /// Update status
        #[arg(short, long)]
        status: Option<String>,

        /// Update owner
        #[arg(short, long)]
        owner: Option<String>,

        /// Add tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        add_tags: Vec<String>,

        /// Remove tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        remove_tags: Vec<String>,

        /// Set due date (YYYY-MM-DD)
        #[arg(long)]
        due: Option<String>,

        /// Clear the due date
        #[arg(long)]
        clear_due: bool,

        /// Priority (high, medium, low)
        #[arg(long)]
        priority: Option<String>,

        /// Flag the card as requiring human intervention (agents skip it)
        #[arg(long)]
        needs_human: bool,

        /// Clear the human-intervention flag
        #[arg(long)]
        clear_needs_human: bool,
    },

    /// List all cards
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by owner
        #[arg(short, long)]
        owner: Option<String>,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Manage checklist items on a card
    Checklist {
        /// Name of the card
        name: String,
        #[command(subcommand)]
        action: ChecklistAction,
    },

    /// Delete a card
    #[command(alias = "delete")]
    Rm {
        /// Name of the card to delete
        name: String,
    },

    /// Show full details of a card
    Show {
        /// Name of the card to show
        name: String,
    },

    /// Search cards by name, description, or tag
    #[command(alias = "search")]
    Find {
        /// Search query
        query: String,
    },

    /// Show summary statistics
    Stats,

    /// Initialise a new cardthing project
    Init,

    /// Enter interactive shell mode
    Shell,

    /// Start a web server rendering the kanban board
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },

    /// Run an agent worker that picks up cards from a column
    Work {
        /// Worker profile name from .cards.toml
        profile: String,

        /// Process at most N cards then exit
        #[arg(long)]
        max_cards: Option<u32>,

        /// Override the agent executable (testing)
        #[arg(long, hide = true)]
        agent_cmd: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ChecklistAction {
    /// Add a new checklist item
    Add {
        /// Text of the item
        text: String,
    },
    /// Toggle a checklist item checked/unchecked (1-based index)
    Toggle {
        /// Item number (from `cardthing show`)
        index: usize,
    },
    /// Remove a checklist item (1-based index)
    Remove {
        /// Item number (from `cardthing show`)
        index: usize,
    },
}
