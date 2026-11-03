use cardthing::cli::{ChecklistAction, Cli, Commands};
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
            blocked_by,
        }) => require_init().and_then(|_| {
            commands::add::execute(
                name,
                description,
                status,
                owner,
                tags,
                due,
                priority,
                blocked_by,
            )
        }),

        Some(Commands::Edit {
            name,
            description,
            status,
            owner,
            clear_owner,
            claim,
            add_tags,
            remove_tags,
            due,
            clear_due,
            priority,
            needs_human,
            clear_needs_human,
            add_blocked_by,
            remove_blocked_by,
        }) => require_init().and_then(|_| {
            commands::edit::execute(
                name,
                commands::edit::EditArgs {
                    description,
                    status,
                    owner,
                    clear_owner,
                    claim,
                    add_tags,
                    remove_tags,
                    due,
                    clear_due,
                    priority,
                    needs_human,
                    clear_needs_human,
                    add_blocked_by,
                    remove_blocked_by,
                },
            )
        }),

        Some(Commands::Append {
            name,
            text,
            section,
        }) => require_init().and_then(|_| commands::append::execute(name, text, section)),

        Some(Commands::Checklist { name, action }) => require_init().and_then(|_| match action {
            ChecklistAction::Add { text } => commands::checklist::add(name, text),
            ChecklistAction::Toggle { index } => commands::checklist::toggle(name, index),
            ChecklistAction::Remove { index } => commands::checklist::remove(name, index),
        }),

        Some(Commands::Rm { name }) => require_init().and_then(|_| commands::remove::execute(name)),

        Some(Commands::Show { name }) => require_init().and_then(|_| commands::show::execute(name)),

        Some(Commands::List {
            status,
            owner,
            tag,
            ready,
            format,
        }) => {
            require_init().and_then(|_| commands::list::execute(status, owner, tag, ready, format))
        }

        Some(Commands::Find { query }) => {
            require_init().and_then(|_| commands::find::execute(query))
        }

        Some(Commands::Snapshot) => require_init().and_then(|_| commands::snapshot::execute()),

        Some(Commands::Stats) => require_init().and_then(|_| commands::stats::execute()),

        Some(Commands::Shell) | None => require_init().and_then(|_| shell::run()),

        Some(Commands::Serve { port }) => {
            require_init().and_then(|_| commands::serve::execute(port))
        }

        Some(Commands::Work {
            profile,
            max_cards,
            watch,
            done,
            prompt_file,
            model,
            effort,
            agent_cmd,
        }) => require_init().and_then(|_| {
            commands::work::execute(
                profile,
                max_cards,
                watch,
                done,
                prompt_file,
                model,
                effort,
                agent_cmd,
            )
        }),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
