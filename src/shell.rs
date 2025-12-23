use crate::commands;
use crate::storage;
use anyhow::{bail, Result};
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

pub fn run() -> Result<()> {
    println!("Welcome to cardthing shell! Type 'help' for commands, 'exit' to quit.");

    let mut rl = DefaultEditor::new()?;
    let mut current_card: Option<String> = None;

    loop {
        let prompt = match &current_card {
            Some(card) => format!("cardthing({})> ", card),
            None => "cardthing> ".to_string(),
        };

        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line)?;

                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                let result = match parts[0] {
                    "exit" | "quit" => {
                        println!("Goodbye!");
                        break;
                    }
                    "help" => {
                        print_help();
                        Ok(())
                    }
                    "touch" | "add" => handle_add(&parts[1..]),
                    "ls" | "list" => handle_list(&parts[1..]),
                    "cd" => handle_cd(&parts[1..], &mut current_card),
                    "set" => handle_set(&parts[1..], &current_card),
                    "edit" => handle_edit(&parts[1..], &current_card),
                    "rm" | "delete" => handle_delete(&parts[1..]),
                    _ => {
                        eprintln!("Unknown command: '{}'. Type 'help' for available commands.", parts[0]);
                        Ok(())
                    }
                };

                if let Err(e) = result {
                    eprintln!("{} {}", "Error:".red(), e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!("\nAvailable commands:");
    println!("  {}  <name> [description]     Create a new card", "touch".cyan());
    println!("  {}  [--status <s>] [--owner <o>]  List all cards", "ls".cyan());
    println!("  {}     <card>                    Enter card context", "cd".cyan());
    println!("  {}    <field> <value>           Set a field (in card context)", "set".cyan());
    println!("  {}   [card] <field> <value>    Edit a card field", "edit".cyan());
    println!("  {}     <card>                    Delete a card", "rm".cyan());
    println!("  {}                           Show this help", "help".cyan());
    println!("  {}                           Exit shell\n", "exit".cyan());
}

fn handle_add(args: &[&str]) -> Result<()> {
    if args.is_empty() {
        bail!("Usage: touch <name> [description]");
    }

    let name = args[0].to_string();
    let description = if args.len() > 1 {
        Some(args[1..].join(" "))
    } else {
        None
    };

    commands::add::execute(name, description, "todo".to_string(), None, vec![])
}

fn handle_list(args: &[&str]) -> Result<()> {
    let mut status = None;
    let mut owner = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--status" | "-s" => {
                if i + 1 < args.len() {
                    status = Some(args[i + 1].to_string());
                    i += 2;
                } else {
                    bail!("--status requires a value");
                }
            }
            "--owner" | "-o" => {
                if i + 1 < args.len() {
                    owner = Some(args[i + 1].to_string());
                    i += 2;
                } else {
                    bail!("--owner requires a value");
                }
            }
            _ => {
                bail!("Unknown option: {}", args[i]);
            }
        }
    }

    commands::list::execute(status, owner, None, "table".to_string())
}

fn handle_cd(args: &[&str], current_card: &mut Option<String>) -> Result<()> {
    if args.is_empty() {
        *current_card = None;
        println!("Left card context");
        return Ok(());
    }

    let card_name = args[0];

    // Check if card exists
    if !storage::card_exists(card_name) {
        bail!("Card '{}' not found", card_name);
    }

    *current_card = Some(card_name.to_string());
    println!("Entered card context: {}", card_name.bold());

    // Show card details
    let card = storage::load_card(card_name)?;
    println!("  Status: {}", card.status.to_string().color(card.status.color()));
    println!("  Owner: {}", card.owner.as_deref().unwrap_or("-"));
    println!("  Description: {}", card.description);
    if !card.tags.is_empty() {
        println!("  Tags: {}", card.tags.join(", "));
    }

    Ok(())
}

fn handle_set(args: &[&str], current_card: &Option<String>) -> Result<()> {
    let card_name = match current_card {
        Some(name) => name.clone(),
        None => bail!("Not in card context. Use 'cd <card>' first or use 'edit <card> <field> <value>'"),
    };

    if args.len() < 2 {
        bail!("Usage: set <field> <value>");
    }

    let field = args[0];
    let value = args[1..].join(" ");

    match field {
        "status" => {
            commands::edit::execute(
                card_name,
                None,
                Some(value),
                None,
                vec![],
                vec![],
            )
        }
        "owner" => {
            commands::edit::execute(
                card_name,
                None,
                None,
                Some(value),
                vec![],
                vec![],
            )
        }
        "description" | "desc" => {
            commands::edit::execute(
                card_name,
                Some(value),
                None,
                None,
                vec![],
                vec![],
            )
        }
        _ => bail!("Unknown field: '{}'. Valid fields: status, owner, description", field),
    }
}

fn handle_edit(args: &[&str], current_card: &Option<String>) -> Result<()> {
    if args.is_empty() {
        bail!("Usage: edit [card] <field> <value>");
    }

    // If in card context, first arg is field, otherwise it's card name
    let (card_name, field, value) = if current_card.is_some() {
        if args.len() < 2 {
            bail!("Usage: edit <field> <value>");
        }
        (
            current_card.clone().unwrap(),
            args[0],
            args[1..].join(" "),
        )
    } else {
        if args.len() < 3 {
            bail!("Usage: edit <card> <field> <value>");
        }
        (args[0].to_string(), args[1], args[2..].join(" "))
    };

    match field {
        "status" => {
            commands::edit::execute(
                card_name,
                None,
                Some(value),
                None,
                vec![],
                vec![],
            )
        }
        "owner" => {
            commands::edit::execute(
                card_name,
                None,
                None,
                Some(value),
                vec![],
                vec![],
            )
        }
        "description" | "desc" => {
            commands::edit::execute(
                card_name,
                Some(value),
                None,
                None,
                vec![],
                vec![],
            )
        }
        _ => bail!("Unknown field: '{}'. Valid fields: status, owner, description", field),
    }
}

fn handle_delete(args: &[&str]) -> Result<()> {
    if args.is_empty() {
        bail!("Usage: rm <card>");
    }

    let card_name = args[0];
    storage::delete_card(card_name)?;
    println!("{} Deleted card '{}'", "✓".green(), card_name.bold());
    Ok(())
}
