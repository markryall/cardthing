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

    /// Enter interactive shell mode
    Shell,

    /// Start a web server rendering the kanban board
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
}
