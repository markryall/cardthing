use cardthing::cli::{Cli, Commands};
use cardthing::commands;
use cardthing::shell;
use clap::Parser;
use std::process;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Add {
            name,
            description,
            status,
            owner,
            tags,
        }) => commands::add::execute(name, description, status, owner, tags),

        Some(Commands::Edit {
            name,
            description,
            status,
            owner,
            add_tags,
            remove_tags,
        }) => commands::edit::execute(name, description, status, owner, add_tags, remove_tags),

        Some(Commands::List {
            status,
            owner,
            tag,
            format,
        }) => commands::list::execute(status, owner, tag, format),

        Some(Commands::Shell) | None => {
            // Enter shell mode if no command specified or explicit shell command
            shell::run()
        }

        Some(Commands::Serve { port }) => commands::serve::execute(port),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
