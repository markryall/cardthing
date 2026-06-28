use cardthing::cli::{Cli, Commands};
use cardthing::commands;
use cardthing::shell;
use clap::Parser;
use std::process;

fn require_init() -> anyhow::Result<()> {
    if std::fs::metadata(".cards").is_err() || std::fs::metadata(".cards.toml").is_err() {
        anyhow::bail!("Not initialised. Run `cardthing init` first.");
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Init) => commands::init::execute(),

        Some(Commands::Add {
            name,
            description,
            status,
            owner,
            tags,
            due,
            priority,
        }) => require_init().and_then(|_| {
            commands::add::execute(name, description, status, owner, tags, due, priority)
        }),

        Some(Commands::Edit {
            name,
            description,
            status,
            owner,
            add_tags,
            remove_tags,
            due,
            clear_due,
            priority,
        }) => require_init().and_then(|_| {
            commands::edit::execute(
                name,
                description,
                status,
                owner,
                add_tags,
                remove_tags,
                due,
                clear_due,
                priority,
            )
        }),

        Some(Commands::Rm { name }) => {
            require_init().and_then(|_| commands::remove::execute(name))
        }

        Some(Commands::Show { name }) => {
            require_init().and_then(|_| commands::show::execute(name))
        }

        Some(Commands::List {
            status,
            owner,
            tag,
            format,
        }) => require_init().and_then(|_| commands::list::execute(status, owner, tag, format)),

        Some(Commands::Shell) | None => require_init().and_then(|_| shell::run()),

        Some(Commands::Serve { port }) => {
            require_init().and_then(|_| commands::serve::execute(port))
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
