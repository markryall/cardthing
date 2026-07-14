use crate::commands::parse_due_date;
use crate::models::{Card, ChecklistItem, Config};
use crate::storage;
use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html,
    },
    routing::get,
    Json, Router,
};
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<()>,
}

const COLUMN_COLORS: &[&str] = &[
    "#f59e0b", "#3b82f6", "#10b981", "#ef4444", "#a855f7", "#f97316", "#06b6d4", "#ec4899",
    "#84cc16", "#14b8a6",
];

pub fn execute(port: u16) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(serve(port))
}

async fn serve(port: u16) -> Result<()> {
    let (tx, _) = broadcast::channel::<()>(16);

    let tx_watch = tx.clone();
    std::thread::spawn(move || watch_cards_dir(tx_watch));

    let state = Arc::new(AppState { tx });

    let app = Router::new()
        .route("/", get(board_handler))
        .route("/events", get(sse_handler))
        .route("/cards", axum::routing::post(post_card))
        .route(
            "/cards/:name/status",
            axum::routing::patch(patch_card_status),
        )
        .route(
            "/cards/:name/checklist/:index",
            axum::routing::patch(toggle_checklist_item),
        )
        .route("/cards/:name", axum::routing::patch(patch_card))
        .route("/columns", axum::routing::post(post_column))
        .route("/columns/order", axum::routing::put(put_column_order))
        .route(
            "/columns/:id",
            axum::routing::patch(patch_column).delete(delete_column),
        )
        .route(
            "/columns/:id/cards/order",
            axum::routing::put(put_column_cards_order),
        )
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Cardthing board running at http://localhost:{}", port);
    println!("Press Ctrl-C to stop.");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn watch_cards_dir(tx: broadcast::Sender<()>) {
    use std::sync::mpsc;
    use std::time::Duration;

    let _ = storage::list_cards();

    let (stx, srx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = match RecommendedWatcher::new(stx, notify::Config::default()) {
        Ok(w) => w,
        Err(_) => return,
    };
    if watcher
        .watch(std::path::Path::new(".cards"), RecursiveMode::NonRecursive)
        .is_err()
    {
        return;
    }

    loop {
        match srx.recv() {
            Ok(Ok(_)) => {
                std::thread::sleep(Duration::from_millis(50));
                while srx.try_recv().is_ok() {}
                let _ = tx.send(());
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.tx.subscribe())
        .map(|_| Ok(Event::default().event("refresh").data("")));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn board_handler() -> Html<String> {
    let html = match storage::list_cards() {
        Ok(mut cards) => {
            cards.sort_by_key(|c| c.created_at);
            render_board(&cards)
        }
        Err(e) => format!(
            "<!DOCTYPE html><html><body><p>Error loading cards: {}</p></body></html>",
            escape_html(&e.to_string())
        ),
    };
    Html(html)
}

// ── API types ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ApiResponse {
    fn ok() -> Json<Self> {
        Json(Self {
            ok: true,
            error: None,
        })
    }
    fn err(e: impl ToString) -> Json<Self> {
        Json(Self {
            ok: false,
            error: Some(e.to_string()),
        })
    }
}

#[derive(Deserialize)]
struct StatusUpdate {
    status: String,
}

#[derive(Deserialize)]
struct ChecklistItemInput {
    text: String,
    checked: bool,
}

#[derive(Deserialize)]
struct CardUpdate {
    description: String,
    status: String,
    owner: Option<String>,
    tags: Vec<String>,
    checklist: Vec<ChecklistItemInput>,
    due_at: Option<String>,
    priority: Option<String>,
    #[serde(default)]
    needs_human: bool,
}

#[derive(Deserialize)]
struct NewCardBody {
    name: String,
    description: String,
    status: String,
    owner: Option<String>,
    tags: Vec<String>,
    checklist: Vec<ChecklistItemInput>,
    due_at: Option<String>,
    priority: Option<String>,
    #[serde(default)]
    needs_human: bool,
}

async fn patch_card_status(
    Path(name): Path<String>,
    Json(body): Json<StatusUpdate>,
) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let config = Config::load();
        let mut card = storage::load_card(&name)?;
        card.status = config.validate_status(&body.status)?;
        card.updated_at = Utc::now();
        storage::save_card(&card)
    })();
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

async fn toggle_checklist_item(Path((name, index)): Path<(String, usize)>) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let mut card = storage::load_card(&name)?;
        let item = card
            .checklist
            .get_mut(index)
            .ok_or_else(|| anyhow::anyhow!("Checklist item {} not found", index))?;
        item.checked = !item.checked;
        card.updated_at = Utc::now();
        storage::save_card(&card)
    })();
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

async fn patch_card(Path(name): Path<String>, Json(body): Json<CardUpdate>) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let config = Config::load();
        let mut card = storage::load_card(&name)?;
        card.description = body.description;
        card.status = config.validate_status(&body.status)?;
        card.owner = body.owner.filter(|o| !o.is_empty());
        card.tags = body.tags;
        card.checklist = body
            .checklist
            .into_iter()
            .filter(|i| !i.text.trim().is_empty())
            .map(|i| ChecklistItem {
                text: i.text,
                checked: i.checked,
            })
            .collect();
        card.due_at = body
            .due_at
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(parse_due_date)
            .transpose()?;
        card.priority = body.priority.filter(|p| !p.is_empty());
        card.needs_human = body.needs_human;
        card.updated_at = Utc::now();
        storage::save_card(&card)
    })();
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

async fn post_card(Json(body): Json<NewCardBody>) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        if storage::card_exists(&body.name) {
            anyhow::bail!("Card '{}' already exists", body.name);
        }
        let config = Config::load();
        let mut card = Card::new(body.name, body.description);
        card.status = config.validate_status(&body.status)?;
        card.owner = body.owner.filter(|o| !o.is_empty());
        card.tags = body.tags;
        card.checklist = body
            .checklist
            .into_iter()
            .filter(|i| !i.text.trim().is_empty())
            .map(|i| ChecklistItem {
                text: i.text,
                checked: i.checked,
            })
            .collect();
        card.due_at = body
            .due_at
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(parse_due_date)
            .transpose()?;
        card.priority = body.priority.filter(|p| !p.is_empty());
        card.needs_human = body.needs_human;
        card.validate()?;
        storage::save_card(&card)
    })();
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

// ── Column handlers ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct NewColumnBody {
    label: String,
}

#[derive(Deserialize)]
struct ColumnUpdate {
    label: String,
}

async fn post_column(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewColumnBody>,
) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let label = body.label.trim().to_string();
        if label.is_empty() {
            anyhow::bail!("Column name is required");
        }
        let mut config = Config::load();
        let id: String = label
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect();
        if id.is_empty() {
            anyhow::bail!("Invalid column name");
        }
        if config.statuses.iter().any(|s| s.id == id) {
            anyhow::bail!("A column with that name already exists");
        }
        let color = COLUMN_COLORS[config.statuses.len() % COLUMN_COLORS.len()].to_string();
        config
            .statuses
            .push(crate::models::StatusDef { id, label, color });
        config.save()
    })();
    if result.is_ok() {
        let _ = state.tx.send(());
    }
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

async fn patch_column(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ColumnUpdate>,
) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let label = body.label.trim().to_string();
        if label.is_empty() {
            anyhow::bail!("Column name is required");
        }
        let mut config = Config::load();
        let status = config
            .statuses
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow::anyhow!("Column '{}' not found", id))?;
        status.label = label;
        config.save()
    })();
    if result.is_ok() {
        let _ = state.tx.send(());
    }
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

#[derive(Deserialize)]
struct ColumnOrder {
    order: Vec<String>,
}

async fn put_column_order(Json(body): Json<ColumnOrder>) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let mut config = Config::load();
        if body.order.len() != config.statuses.len() {
            anyhow::bail!("Order must include all columns");
        }
        let mut reordered = Vec::with_capacity(config.statuses.len());
        for id in &body.order {
            let status = config
                .statuses
                .iter()
                .find(|s| &s.id == id)
                .ok_or_else(|| anyhow::anyhow!("Unknown column '{}'", id))?
                .clone();
            reordered.push(status);
        }
        config.statuses = reordered;
        config.save()
    })();
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

#[derive(Deserialize)]
struct ColumnCardsOrder {
    names: Vec<String>,
}

async fn put_column_cards_order(
    Path(id): Path<String>,
    Json(body): Json<ColumnCardsOrder>,
) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        for (i, name) in body.names.iter().enumerate() {
            let mut card = storage::load_card(name)?;
            if card.status != id {
                anyhow::bail!("Card '{}' is not in column '{}'", name, id);
            }
            card.order = Some(i as u32);
            card.updated_at = Utc::now();
            storage::save_card(&card)?;
        }
        Ok(())
    })();
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

async fn delete_column(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let cards = storage::list_cards()?;
        let count = cards.iter().filter(|c| c.status == id).count();
        if count > 0 {
            anyhow::bail!("Move or delete the {} card(s) in this column first", count);
        }
        let mut config = Config::load();
        let before = config.statuses.len();
        config.statuses.retain(|s| s.id != id);
        if config.statuses.len() == before {
            anyhow::bail!("Column '{}' not found", id);
        }
        config.save()
    })();
    if result.is_ok() {
        let _ = state.tx.send(());
    }
    match result {
        Ok(_) => ApiResponse::ok(),
        Err(e) => ApiResponse::err(e),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render_board(cards: &[Card]) -> String {
    let config = Config::load();

    let columns_html: String = config
        .statuses
        .iter()
        .map(|col| {
            let mut col_cards: Vec<&Card> = cards.iter().filter(|c| c.status == col.id).collect();
            col_cards.sort_by(|a, b| {
                let ao = a.order.unwrap_or(u32::MAX);
                let bo = b.order.unwrap_or(u32::MAX);
                ao.cmp(&bo).then_with(|| a.created_at.cmp(&b.created_at))
            });
            render_column(&col.id, &col.label, &col.color, &col_cards)
        })
        .collect();

    let status_options: String = config
        .statuses
        .iter()
        .map(|s| {
            format!(
                r#"<option value="{}">{}</option>"#,
                escape_html(&s.id),
                escape_html(&s.label)
            )
        })
        .collect();

    let default_status = escape_html(
        config
            .statuses
            .first()
            .map(|s| s.id.as_str())
            .unwrap_or("todo"),
    );
    let title = escape_html(&config.title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; }}
  header {{ padding: 1.5rem 2rem; border-bottom: 1px solid #1e293b; display: flex; align-items: center; gap: 0.75rem; }}
  header h1 {{ font-size: 1.25rem; font-weight: 600; letter-spacing: -0.01em; color: #f8fafc; }}
  header .count {{ font-size: 0.8rem; color: #64748b; flex: 1; }}
  .btn-new {{ background: #3b82f6; color: #fff; border: none; border-radius: 0.375rem; cursor: pointer; font-size: 0.8rem; font-weight: 500; padding: 0.4rem 0.875rem; }}
  .btn-new:hover {{ background: #2563eb; }}
  .btn-outline {{ background: transparent; color: #64748b; border: 1px solid #334155; border-radius: 0.375rem; cursor: pointer; font-size: 0.8rem; font-weight: 500; padding: 0.4rem 0.875rem; }}
  .btn-outline:hover {{ color: #f1f5f9; border-color: #64748b; }}
  .board {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 1rem; padding: 1.5rem 2rem; align-items: start; }}
  .column {{ background: #1e293b; border-radius: 0.75rem; padding: 1rem; }}
  .column-header {{ display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; cursor: grab; }}
  .column-header:active {{ cursor: grabbing; }}
  .btn-col-edit, .btn-col-del, .column-count {{ cursor: default; }}
  .column-label {{ font-size: 0.8rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; }}
  .column-actions {{ display: flex; align-items: center; gap: 0.25rem; }}
  .column-count {{ font-size: 0.75rem; background: #0f172a; color: #94a3b8; border-radius: 999px; padding: 0.1rem 0.5rem; font-weight: 500; }}
  .btn-col-edit, .btn-col-del {{ background: none; border: none; color: #334155; cursor: pointer; font-size: 0.8rem; padding: 0.1rem 0.2rem; line-height: 1; border-radius: 0.25rem; }}
  .btn-col-edit:hover {{ color: #94a3b8; }}
  .btn-col-del:hover {{ color: #f87171; }}
  .toast {{ display: none; position: fixed; top: 1.5rem; left: 50%; transform: translateX(-50%); background: #1e293b; border: 1px solid #ef4444; color: #fca5a5; border-radius: 0.5rem; padding: 0.625rem 1rem; font-size: 0.8rem; z-index: 300; white-space: nowrap; }}
  .toast.show {{ display: block; }}
  .column-cards {{ min-height: 2rem; }}
  .card {{ background: #0f172a; border-radius: 0.5rem; padding: 0.875rem; margin-bottom: 0.625rem; border: 1px solid #1e293b; cursor: pointer; }}
  .card:last-child {{ margin-bottom: 0; }}
  .card-name {{ font-size: 0.875rem; font-weight: 600; color: #f1f5f9; margin-bottom: 0.375rem; }}
  .card-desc {{ font-size: 0.775rem; color: #94a3b8; line-height: 1.4; margin-bottom: 0.5rem; }}
  .card-meta {{ display: flex; flex-wrap: wrap; gap: 0.375rem; align-items: center; }}
  .owner {{ font-size: 0.7rem; color: #cbd5e1; background: #1e293b; border-radius: 999px; padding: 0.1rem 0.5rem; }}
  .tag {{ font-size: 0.7rem; color: #94a3b8; background: #0f172a; border: 1px solid #334155; border-radius: 999px; padding: 0.1rem 0.5rem; }}
  .empty {{ font-size: 0.775rem; color: #475569; text-align: center; padding: 1.5rem 0; }}
  .sortable-ghost {{ opacity: 0.3; }}
  .sortable-drag {{ opacity: 0.9; box-shadow: 0 8px 24px rgba(0,0,0,0.4); }}
  /* Keyboard focus */
  .card.focused {{ border-color: #3b82f6; box-shadow: 0 0 0 2px rgba(59,130,246,0.25); }}
  /* Shortcuts panel */
  .shortcuts-panel {{ display: none; position: fixed; bottom: 1.5rem; right: 1.5rem; background: #1e293b; border: 1px solid #334155; border-radius: 0.75rem; padding: 1rem 1.25rem; z-index: 50; min-width: 220px; }}
  .shortcuts-panel.open {{ display: block; }}
  .shortcuts-panel h3 {{ font-size: 0.75rem; font-weight: 600; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.06em; margin-bottom: 0.625rem; }}
  .shortcuts-panel table {{ border-collapse: collapse; width: 100%; }}
  .shortcuts-panel td {{ font-size: 0.775rem; padding: 0.15rem 0; color: #94a3b8; }}
  .shortcuts-panel td:first-child {{ padding-right: 1rem; white-space: nowrap; }}
  kbd {{ background: #0f172a; border: 1px solid #334155; border-radius: 0.25rem; font-size: 0.7rem; padding: 0.1rem 0.35rem; color: #cbd5e1; font-family: inherit; }}
  .btn-help {{ position: fixed; bottom: 1.5rem; right: 1.5rem; background: #1e293b; border: 1px solid #334155; border-radius: 999px; color: #64748b; cursor: pointer; font-size: 0.8rem; width: 2rem; height: 2rem; display: flex; align-items: center; justify-content: center; z-index: 49; }}
  .btn-help:hover {{ color: #f1f5f9; border-color: #64748b; }}
  /* Checklist progress on card */
  .checklist-progress {{ font-size: 0.7rem; color: #64748b; margin-top: 0.5rem; display: flex; align-items: center; gap: 0.375rem; }}
  .progress-bar {{ flex: 1; height: 3px; background: #1e293b; border-radius: 999px; overflow: hidden; }}
  .progress-fill {{ height: 100%; background: #10b981; border-radius: 999px; }}
  /* Modal */
  .backdrop {{ display: none; position: fixed; inset: 0; background: rgba(0,0,0,0.6); z-index: 100; align-items: center; justify-content: center; }}
  .backdrop.open {{ display: flex; }}
  .modal {{ background: #1e293b; border-radius: 0.75rem; padding: 1.5rem; width: 100%; max-width: 480px; margin: 1rem; max-height: 90vh; overflow-y: auto; }}
  .modal-header {{ display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.25rem; }}
  .modal-title {{ font-size: 1rem; font-weight: 600; color: #f1f5f9; }}
  .modal-close {{ background: none; border: none; color: #64748b; cursor: pointer; font-size: 1.5rem; line-height: 1; padding: 0; }}
  .modal-close:hover {{ color: #f1f5f9; }}
  .field {{ margin-bottom: 0.875rem; }}
  .field label {{ display: block; font-size: 0.75rem; font-weight: 500; color: #94a3b8; margin-bottom: 0.3rem; text-transform: uppercase; letter-spacing: 0.04em; }}
  .field input[type=text], .field textarea, .field select {{ width: 100%; background: #0f172a; border: 1px solid #334155; border-radius: 0.375rem; color: #f1f5f9; font-size: 0.875rem; padding: 0.5rem 0.625rem; font-family: inherit; }}
  .field input[type=text]:focus, .field textarea:focus, .field select:focus {{ outline: none; border-color: #3b82f6; }}
  .field textarea {{ resize: vertical; min-height: 72px; }}
  .field select {{ appearance: none; cursor: pointer; }}
  /* Checklist in modal */
  .cl-items {{ display: flex; flex-direction: column; gap: 0.375rem; margin-bottom: 0.5rem; }}
  .cl-row {{ display: flex; align-items: center; gap: 0.375rem; }}
  .cl-row input[type=checkbox] {{ cursor: pointer; accent-color: #10b981; flex-shrink: 0; }}
  .cl-row input[type=text] {{ flex: 1; background: #0f172a; border: 1px solid #334155; border-radius: 0.375rem; color: #f1f5f9; font-size: 0.8rem; padding: 0.375rem 0.5rem; font-family: inherit; }}
  .cl-row input[type=text]:focus {{ outline: none; border-color: #3b82f6; }}
  .cl-del {{ background: none; border: none; color: #475569; cursor: pointer; font-size: 1rem; line-height: 1; padding: 0 0.25rem; flex-shrink: 0; }}
  .cl-del:hover {{ color: #f87171; }}
  .btn-add-item {{ background: none; border: 1px dashed #334155; border-radius: 0.375rem; color: #64748b; cursor: pointer; font-size: 0.775rem; padding: 0.375rem 0.625rem; width: 100%; text-align: left; }}
  .btn-add-item:hover {{ border-color: #64748b; color: #94a3b8; }}
  .modal-error {{ color: #f87171; font-size: 0.775rem; min-height: 1rem; margin-top: 0.25rem; }}
  .modal-footer {{ display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1.25rem; }}
  .btn {{ border: none; border-radius: 0.375rem; cursor: pointer; font-size: 0.875rem; font-weight: 500; padding: 0.5rem 1rem; }}
  .btn-ghost {{ background: transparent; color: #94a3b8; border: 1px solid #334155; }}
  .btn-ghost:hover {{ color: #f1f5f9; border-color: #64748b; }}
  .btn-primary {{ background: #3b82f6; color: #fff; }}
  .btn-primary:hover {{ background: #2563eb; }}
  @media (max-width: 500px) {{ .board {{ grid-template-columns: 1fr; }} }}
  .priority-badge {{ font-size: 0.7rem; border-radius: 999px; padding: 0.1rem 0.5rem; font-weight: 600; }}
  .priority-high {{ background: rgba(239,68,68,0.15); color: #f87171; }}
  .priority-medium {{ background: rgba(245,158,11,0.15); color: #fbbf24; }}
  .priority-low {{ background: rgba(16,185,129,0.15); color: #34d399; }}
  .due-badge {{ font-size: 0.7rem; color: #94a3b8; background: #1e293b; border-radius: 999px; padding: 0.1rem 0.5rem; }}
  .due-overdue {{ color: #f87171; background: rgba(239,68,68,0.1); }}
  .field input[type=date] {{ width: 100%; background: #0f172a; border: 1px solid #334155; border-radius: 0.375rem; color: #f1f5f9; font-size: 0.875rem; padding: 0.5rem 0.625rem; font-family: inherit; color-scheme: dark; }}
  .field input[type=date]:focus {{ outline: none; border-color: #3b82f6; }}
  /* Agent worker indicators */
  .card.agent-working {{ border: 1px dashed #3b82f6; }}
  .card.needs-human {{ border: 1px solid #f59e0b; }}
  .agent-badge {{ font-size: 0.7rem; color: #93c5fd; background: rgba(59,130,246,0.15); border-radius: 999px; padding: 0.1rem 0.5rem; display: inline-flex; align-items: center; gap: 0.3rem; }}
  .agent-dot {{ width: 0.4rem; height: 0.4rem; border-radius: 999px; background: #3b82f6; animation: agent-pulse 1.2s ease-in-out infinite; }}
  @keyframes agent-pulse {{ 0%, 100% {{ opacity: 1; }} 50% {{ opacity: 0.25; }} }}
  .needs-human-badge {{ font-size: 0.7rem; color: #fbbf24; background: rgba(245,158,11,0.15); border-radius: 999px; padding: 0.1rem 0.5rem; font-weight: 600; }}
  .field-check {{ display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.875rem; }}
  .field-check input[type=checkbox] {{ cursor: pointer; accent-color: #f59e0b; }}
  .field-check label {{ font-size: 0.8rem; color: #94a3b8; cursor: pointer; }}
</style>
</head>
<body>
<header>
  <h1>{title}</h1>
  <span class="count">{total} cards</span>
  <button class="btn-outline" onclick="openAddColumn()">+ Column</button>
  <button class="btn-new" onclick="openCreate()">+ New Card</button>
</header>
<div class="board">
{columns}
</div>

<div class="backdrop" id="backdrop" onclick="backdropClick(event)">
  <div class="modal">
    <div class="modal-header">
      <span class="modal-title" id="modal-title">New Card</span>
      <button class="modal-close" onclick="closeModal()">&#x2715;</button>
    </div>
    <div class="field" id="name-field">
      <label>Name</label>
      <input id="f-name" type="text" placeholder="card-name">
    </div>
    <div class="field">
      <label>Description</label>
      <textarea id="f-desc" placeholder="What needs to be done?"></textarea>
    </div>
    <div class="field">
      <label>Status</label>
      <select id="f-status">{status_options}</select>
    </div>
    <div class="field">
      <label>Owner</label>
      <input id="f-owner" type="text" placeholder="someone">
    </div>
    <div class="field">
      <label>Tags</label>
      <input id="f-tags" type="text" placeholder="comma-separated">
    </div>
    <div class="field">
      <label>Due Date</label>
      <input id="f-due" type="date">
    </div>
    <div class="field">
      <label>Priority</label>
      <select id="f-priority">
        <option value="">None</option>
        <option value="high">High</option>
        <option value="medium">Medium</option>
        <option value="low">Low</option>
      </select>
    </div>
    <div class="field-check">
      <input id="f-needs-human" type="checkbox">
      <label for="f-needs-human">🙋 Needs human intervention (agents will skip this card)</label>
    </div>
    <div class="field">
      <label>Checklist</label>
      <div class="cl-items" id="cl-items"></div>
      <button class="btn-add-item" onclick="addChecklistRow()">+ Add item</button>
    </div>
    <div class="modal-error" id="modal-error"></div>
    <div class="modal-footer">
      <button class="btn btn-ghost" onclick="closeModal()">Cancel</button>
      <button class="btn btn-primary" id="modal-submit" onclick="submitModal()">Create</button>
    </div>
  </div>
</div>

<div class="backdrop" id="col-backdrop" onclick="colBackdropClick(event)">
  <div class="modal">
    <div class="modal-header">
      <span class="modal-title" id="col-modal-title">Add Column</span>
      <button class="modal-close" onclick="closeColModal()">&#x2715;</button>
    </div>
    <div class="field">
      <label>Name</label>
      <input id="col-f-label" type="text" placeholder="Column name">
    </div>
    <div class="modal-error" id="col-modal-error"></div>
    <div class="modal-footer">
      <button class="btn btn-ghost" onclick="closeColModal()">Cancel</button>
      <button class="btn btn-primary" id="col-modal-submit" onclick="submitColModal()">Add</button>
    </div>
  </div>
</div>
<div id="toast" class="toast"></div>

<div class="shortcuts-panel" id="shortcuts-panel">
  <h3>Board</h3>
  <table>
    <tr><td><kbd>n</kbd></td><td>New card</td></tr>
    <tr><td><kbd>Enter</kbd></td><td>Edit focused card</td></tr>
    <tr><td><kbd>j</kbd> <kbd>k</kbd></td><td>Move focus up / down</td></tr>
    <tr><td><kbd>h</kbd> <kbd>l</kbd></td><td>Move focus left / right</td></tr>
    <tr><td><kbd>[</kbd> <kbd>]</kbd></td><td>Move card left / right</td></tr>
    <tr><td><kbd>Esc</kbd></td><td>Deselect / close</td></tr>
    <tr><td><kbd>?</kbd></td><td>Toggle this panel</td></tr>
  </table>
  <h3 style="margin-top:0.75rem">Checklist (in modal)</h3>
  <table>
    <tr><td><kbd>Enter</kbd></td><td>New item below</td></tr>
    <tr><td><kbd>↑</kbd> <kbd>↓</kbd></td><td>Move between items</td></tr>
    <tr><td><kbd>Ctrl</kbd>+<kbd>Space</kbd></td><td>Toggle checked</td></tr>
    <tr><td><kbd>⌫</kbd> on empty</td><td>Delete item</td></tr>
  </table>
</div>
<button class="btn-help" id="btn-help" onclick="toggleHelp()" title="Keyboard shortcuts">?</button>

<script src="https://cdn.jsdelivr.net/npm/sortablejs@1.15.6/Sortable.min.js"></script>
<script>
  const DEFAULT_STATUS = '{default_status}';
  let modalMode = null;
  let editCardName = null;
  let dragging = false;
  let pendingRefresh = false;

  // ── Live reload via SSE ──────────────────────────────────────────────────
  const evtSource = new EventSource('/events');
  evtSource.addEventListener('refresh', () => {{
    const anyOpen = document.getElementById('backdrop').classList.contains('open') ||
                    document.getElementById('col-backdrop').classList.contains('open');
    if (anyOpen) {{ pendingRefresh = true; }} else {{ location.reload(); }}
  }});

  // ── Column modal ─────────────────────────────────────────────────────────
  let colModalMode = null;
  let editColId = null;

  function openAddColumn() {{
    colModalMode = 'add';
    editColId = null;
    document.getElementById('col-modal-title').textContent = 'Add Column';
    document.getElementById('col-modal-submit').textContent = 'Add';
    document.getElementById('col-f-label').value = '';
    document.getElementById('col-modal-error').textContent = '';
    document.getElementById('col-backdrop').classList.add('open');
    document.getElementById('col-f-label').focus();
  }}

  function openEditColumn(id, label) {{
    colModalMode = 'edit';
    editColId = id;
    document.getElementById('col-modal-title').textContent = 'Rename Column';
    document.getElementById('col-modal-submit').textContent = 'Save';
    document.getElementById('col-f-label').value = label;
    document.getElementById('col-modal-error').textContent = '';
    document.getElementById('col-backdrop').classList.add('open');
    document.getElementById('col-f-label').focus();
    document.getElementById('col-f-label').select();
  }}

  function closeColModal() {{
    document.getElementById('col-backdrop').classList.remove('open');
    if (pendingRefresh) {{ pendingRefresh = false; location.reload(); }}
  }}

  function colBackdropClick(evt) {{
    if (evt.target === document.getElementById('col-backdrop')) closeColModal();
  }}

  async function submitColModal() {{
    const errorEl = document.getElementById('col-modal-error');
    errorEl.textContent = '';
    const label = document.getElementById('col-f-label').value.trim();
    if (!label) {{ errorEl.textContent = 'Name is required'; return; }}
    const btn = document.getElementById('col-modal-submit');
    btn.disabled = true;
    const url = colModalMode === 'add' ? '/columns' : `/columns/${{encodeURIComponent(editColId)}}`;
    const method = colModalMode === 'add' ? 'POST' : 'PATCH';
    try {{
      const r = await fetch(url, {{ method, headers: {{ 'Content-Type': 'application/json' }}, body: JSON.stringify({{ label }}) }});
      const data = await r.json();
      if (data.ok) {{ closeColModal(); }} else {{ errorEl.textContent = data.error || 'Something went wrong'; btn.disabled = false; }}
    }} catch (e) {{ errorEl.textContent = 'Network error'; btn.disabled = false; }}
  }}

  async function deleteColumn(id) {{
    const r = await fetch(`/columns/${{encodeURIComponent(id)}}`, {{ method: 'DELETE' }});
    const data = await r.json();
    if (!data.ok) showToast(data.error || 'Could not delete column');
  }}

  function showToast(msg) {{
    const t = document.getElementById('toast');
    t.textContent = msg;
    t.classList.add('show');
    setTimeout(() => t.classList.remove('show'), 4000);
  }}

  document.getElementById('col-f-label').addEventListener('keydown', e => {{
    if (e.key === 'Enter') {{ e.preventDefault(); submitColModal(); }}
  }});

  // ── Drag and drop ────────────────────────────────────────────────────────
  new Sortable(document.querySelector('.board'), {{
    animation: 150,
    handle: '.column-header',
    draggable: '.column',
    ghostClass: 'sortable-ghost',
    dragClass: 'sortable-drag',
    onEnd(evt) {{
      if (evt.oldIndex === evt.newIndex) return;
      const order = Array.from(document.querySelectorAll('.column')).map(c => c.dataset.status);
      fetch('/columns/order', {{
        method: 'PUT',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ order }}),
      }}).catch(() => location.reload());
    }}
  }});

  document.querySelectorAll('.column').forEach(col => {{
    new Sortable(col.querySelector('.column-cards'), {{
      group: 'cards',
      animation: 150,
      ghostClass: 'sortable-ghost',
      dragClass: 'sortable-drag',
      onStart() {{ dragging = true; }},
      onEnd(evt) {{
        setTimeout(() => {{ dragging = false; }}, 0);
        if (evt.from === evt.to) {{
          if (evt.oldIndex === evt.newIndex) return;
          const status = evt.to.closest('.column').dataset.status;
          const names = Array.from(evt.to.querySelectorAll('.card')).map(c => c.dataset.name);
          fetch(`/columns/${{encodeURIComponent(status)}}/cards/order`, {{
            method: 'PUT',
            headers: {{ 'Content-Type': 'application/json' }},
            body: JSON.stringify({{ names }}),
          }}).catch(() => {{}});
          return;
        }}
        const name = evt.item.dataset.name;
        const status = evt.to.closest('.column').dataset.status;
        fetch(`/cards/${{encodeURIComponent(name)}}/status`, {{
          method: 'PATCH',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{ status }}),
        }})
          .then(r => r.json())
          .then(data => {{
            if (data.ok) {{
              evt.item.dataset.status = status;
            }} else {{
              location.reload();
            }}
          }})
          .catch(() => location.reload());
      }}
    }});
  }});

  // ── Checklist modal helpers ───────────────────────────────────────────────
  function addChecklistRow(text = '', checked = false, afterRow = null) {{
    const container = document.getElementById('cl-items');
    const row = document.createElement('div');
    row.className = 'cl-row';
    row.innerHTML = `
      <input type="checkbox" ${{checked ? 'checked' : ''}}>
      <input type="text" value="${{escapeAttr(text)}}" placeholder="Item text">
      <button class="cl-del" onclick="this.closest('.cl-row').remove()" title="Remove">&#x2715;</button>
    `;
    const textInput = row.querySelector('input[type=text]');
    textInput.addEventListener('keydown', e => {{
      const rows = Array.from(container.querySelectorAll('.cl-row'));
      const idx = rows.indexOf(row);
      if (e.key === 'Enter') {{
        e.preventDefault();
        addChecklistRow('', false, row);
      }} else if (e.key === 'Backspace' && textInput.value === '') {{
        e.preventDefault();
        row.remove();
        const prev = rows[idx - 1];
        if (prev) prev.querySelector('input[type=text]').focus();
      }} else if (e.key === ' ' && e.ctrlKey) {{
        e.preventDefault();
        const cb = row.querySelector('input[type=checkbox]');
        cb.checked = !cb.checked;
      }} else if (e.key === 'ArrowUp' && idx > 0) {{
        e.preventDefault();
        rows[idx - 1].querySelector('input[type=text]').focus();
      }} else if (e.key === 'ArrowDown' && idx < rows.length - 1) {{
        e.preventDefault();
        rows[idx + 1].querySelector('input[type=text]').focus();
      }}
    }});
    if (afterRow && afterRow.nextSibling) {{
      container.insertBefore(row, afterRow.nextSibling);
    }} else {{
      container.appendChild(row);
    }}
    textInput.focus();
  }}

  function escapeAttr(s) {{
    return s.replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
  }}

  function getChecklistFromModal() {{
    return Array.from(document.querySelectorAll('#cl-items .cl-row')).map(row => ({{
      text: row.querySelector('input[type=text]').value.trim(),
      checked: row.querySelector('input[type=checkbox]').checked,
    }})).filter(i => i.text);
  }}

  // ── Modal open/close ─────────────────────────────────────────────────────
  function openCreate() {{
    modalMode = 'create';
    editCardName = null;
    document.getElementById('modal-title').textContent = 'New Card';
    document.getElementById('modal-submit').textContent = 'Create';
    document.getElementById('name-field').style.display = '';
    document.getElementById('f-name').value = '';
    document.getElementById('f-desc').value = '';
    document.getElementById('f-status').value = DEFAULT_STATUS;
    document.getElementById('f-owner').value = '';
    document.getElementById('f-tags').value = '';
    document.getElementById('f-due').value = '';
    document.getElementById('f-priority').value = '';
    document.getElementById('f-needs-human').checked = false;
    document.getElementById('cl-items').innerHTML = '';
    document.getElementById('modal-error').textContent = '';
    document.getElementById('backdrop').classList.add('open');
    document.getElementById('f-name').focus();
  }}

  function openEdit(el) {{
    if (dragging) return;
    modalMode = 'edit';
    editCardName = el.dataset.name;
    document.getElementById('modal-title').textContent = el.dataset.name;
    document.getElementById('modal-submit').textContent = 'Save';
    document.getElementById('name-field').style.display = 'none';
    document.getElementById('f-desc').value = el.dataset.description || '';
    document.getElementById('f-status').value = el.dataset.status || DEFAULT_STATUS;
    document.getElementById('f-owner').value = el.dataset.owner || '';
    document.getElementById('f-tags').value = el.dataset.tags || '';
    document.getElementById('f-due').value = el.dataset.due || '';
    document.getElementById('f-priority').value = el.dataset.priority || '';
    document.getElementById('f-needs-human').checked = el.dataset.needsHuman === 'true';
    document.getElementById('cl-items').innerHTML = '';
    const checklist = JSON.parse(el.dataset.checklist || '[]');
    checklist.forEach(item => addChecklistRow(item.text, item.checked));
    document.getElementById('modal-error').textContent = '';
    document.getElementById('backdrop').classList.add('open');
    document.getElementById('f-desc').focus();
  }}

  function closeModal() {{
    document.getElementById('backdrop').classList.remove('open');
    if (pendingRefresh) {{ pendingRefresh = false; location.reload(); }}
  }}

  function backdropClick(evt) {{
    if (evt.target === document.getElementById('backdrop')) closeModal();
  }}

  async function submitModal() {{
    const errorEl = document.getElementById('modal-error');
    errorEl.textContent = '';

    const description = document.getElementById('f-desc').value.trim();
    const status = document.getElementById('f-status').value;
    const owner = document.getElementById('f-owner').value.trim() || null;
    const tags = document.getElementById('f-tags').value
      .split(',').map(t => t.trim()).filter(Boolean);
    const due_at = document.getElementById('f-due').value || null;
    const priority = document.getElementById('f-priority').value || null;
    const needs_human = document.getElementById('f-needs-human').checked;
    const checklist = getChecklistFromModal();

    let url, method, body;
    if (modalMode === 'create') {{
      const name = document.getElementById('f-name').value.trim();
      if (!name) {{ errorEl.textContent = 'Name is required'; return; }}
      url = '/cards';
      method = 'POST';
      body = {{ name, description, status, owner, tags, checklist, due_at, priority, needs_human }};
    }} else {{
      url = `/cards/${{encodeURIComponent(editCardName)}}`;
      method = 'PATCH';
      body = {{ description, status, owner, tags, checklist, due_at, priority, needs_human }};
    }}

    const btn = document.getElementById('modal-submit');
    btn.disabled = true;
    try {{
      const r = await fetch(url, {{
        method,
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify(body),
      }});
      const data = await r.json();
      if (data.ok) {{
        pendingRefresh = false;
        location.reload();
      }} else {{
        errorEl.textContent = data.error || 'Something went wrong';
        btn.disabled = false;
      }}
    }} catch (e) {{
      errorEl.textContent = 'Network error';
      btn.disabled = false;
    }}
  }}

  // ── Keyboard navigation ──────────────────────────────────────────────────
  let focusedCard = null;

  function focusCard(card) {{
    if (focusedCard) focusedCard.classList.remove('focused');
    focusedCard = card;
    if (card) {{
      card.classList.add('focused');
      card.scrollIntoView({{ block: 'nearest', behavior: 'smooth' }});
    }}
  }}

  function getColumns() {{
    return Array.from(document.querySelectorAll('.column'));
  }}

  function getCardsInColumn(col) {{
    return Array.from(col.querySelectorAll('.card'));
  }}

  function moveFocus(dir) {{
    const columns = getColumns();
    if (!columns.length) return;
    if (!focusedCard) {{
      const first = columns[0].querySelector('.card');
      if (first) focusCard(first);
      return;
    }}
    const col = focusedCard.closest('.column');
    const colIdx = columns.indexOf(col);
    const cards = getCardsInColumn(col);
    const cardIdx = cards.indexOf(focusedCard);
    if (dir === 'down') {{
      const next = cards[cardIdx + 1];
      if (next) focusCard(next);
    }} else if (dir === 'up') {{
      const prev = cards[cardIdx - 1];
      if (prev) focusCard(prev);
    }} else if (dir === 'left' && colIdx > 0) {{
      const prevCards = getCardsInColumn(columns[colIdx - 1]);
      const target = prevCards[Math.min(cardIdx, prevCards.length - 1)];
      if (target) focusCard(target);
    }} else if (dir === 'right' && colIdx < columns.length - 1) {{
      const nextCards = getCardsInColumn(columns[colIdx + 1]);
      const target = nextCards[Math.min(cardIdx, nextCards.length - 1)];
      if (target) focusCard(target);
    }}
  }}

  function moveCardToAdjacentColumn(offset) {{
    if (!focusedCard) return;
    const columns = getColumns();
    const col = focusedCard.closest('.column');
    const colIdx = columns.indexOf(col);
    const targetIdx = colIdx + offset;
    if (targetIdx < 0 || targetIdx >= columns.length) return;
    const targetCol = columns[targetIdx];
    const status = targetCol.dataset.status;
    const name = focusedCard.dataset.name;
    fetch(`/cards/${{encodeURIComponent(name)}}/status`, {{
      method: 'PATCH',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify({{ status }}),
    }})
      .then(r => r.json())
      .then(data => {{
        if (data.ok) {{
          focusedCard.dataset.status = status;
          targetCol.querySelector('.column-cards').appendChild(focusedCard);
          updateColumnCounts();
        }} else {{
          location.reload();
        }}
      }})
      .catch(() => location.reload());
  }}

  function updateColumnCounts() {{
    document.querySelectorAll('.column').forEach(col => {{
      const cards = getCardsInColumn(col);
      col.querySelector('.column-count').textContent = cards.length;
      const cardsEl = col.querySelector('.column-cards');
      const empty = cardsEl.querySelector('.empty');
      if (cards.length === 0 && !empty) {{
        const div = document.createElement('div');
        div.className = 'empty';
        div.textContent = 'No cards';
        cardsEl.appendChild(div);
      }} else if (cards.length > 0 && empty) {{
        empty.remove();
      }}
    }});
  }}

  function toggleHelp() {{
    const panel = document.getElementById('shortcuts-panel');
    const btn = document.getElementById('btn-help');
    const open = panel.classList.toggle('open');
    btn.style.display = open ? 'none' : '';
  }}

  document.addEventListener('keydown', e => {{
    const inInput = ['INPUT','TEXTAREA','SELECT'].includes(e.target.tagName);
    const modalOpen = document.getElementById('backdrop').classList.contains('open') ||
                      document.getElementById('col-backdrop').classList.contains('open');

    // Modal shortcuts
    if (modalOpen) {{
      if (e.key === 'Escape') {{
        e.preventDefault();
        if (document.getElementById('col-backdrop').classList.contains('open')) {{ closeColModal(); }} else {{ closeModal(); }}
      }}
      if (e.key === 'Enter' && e.metaKey && !document.getElementById('col-backdrop').classList.contains('open')) {{ e.preventDefault(); submitModal(); }}
      return;
    }}

    // Board shortcuts (ignore when typing in inputs)
    if (inInput) return;

    switch (e.key) {{
      case 'n': e.preventDefault(); openCreate(); break;
      case 'j': case 'ArrowDown':  e.preventDefault(); moveFocus('down');  break;
      case 'k': case 'ArrowUp':    e.preventDefault(); moveFocus('up');    break;
      case 'h': case 'ArrowLeft':  e.preventDefault(); moveFocus('left');  break;
      case 'l': case 'ArrowRight': e.preventDefault(); moveFocus('right'); break;
      case 'Enter': if (focusedCard) {{ e.preventDefault(); openEdit(focusedCard); }} break;
      case '[': e.preventDefault(); moveCardToAdjacentColumn(-1); break;
      case ']': e.preventDefault(); moveCardToAdjacentColumn(+1); break;
      case 'Escape':
        if (focusedCard) {{ focusCard(null); }}
        document.getElementById('shortcuts-panel').classList.remove('open');
        document.getElementById('btn-help').style.display = '';
        break;
      case '?': e.preventDefault(); toggleHelp(); break;
    }}
  }});
</script>
</body>
</html>"#,
        total = cards.len(),
        columns = columns_html,
        status_options = status_options,
        default_status = default_status,
    )
}

fn render_column(status_id: &str, label: &str, color: &str, cards: &[&Card]) -> String {
    let cards_html: String = if cards.is_empty() {
        "<div class=\"empty\">No cards</div>".to_string()
    } else {
        cards.iter().map(|c| render_card(c)).collect()
    };

    let label_json = serde_json::to_string(label).unwrap_or_default();
    let del_btn = if cards.is_empty() {
        format!(
            r#"<button class="btn-col-del" onclick="deleteColumn('{}')" title="Delete">&#x2715;</button>"#,
            escape_html(status_id)
        )
    } else {
        String::new()
    };
    format!(
        r#"<div class="column" data-status="{status}">
  <div class="column-header">
    <span class="column-label" style="color:{color}">{label}</span>
    <div class="column-actions">
      <span class="column-count">{count}</span>
      <button class="btn-col-edit" onclick="openEditColumn('{status}',{label_json})" title="Rename">&#9998;</button>
      {del_btn}
    </div>
  </div>
  <div class="column-cards">
    {cards}
  </div>
</div>"#,
        status = escape_html(status_id),
        color = escape_html(color),
        label = escape_html(label),
        label_json = label_json,
        del_btn = del_btn,
        count = cards.len(),
        cards = cards_html,
    )
}

fn render_card(card: &Card) -> String {
    let desc_display = if card.description.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="card-desc">{}</div>"#,
            escape_html(&truncate(&card.description, 120))
        )
    };

    let meta_html: String = {
        let mut parts = Vec::new();
        if card.agent {
            parts.push(
                r#"<span class="agent-badge"><span class="agent-dot"></span>agent</span>"#
                    .to_string(),
            );
        }
        if card.needs_human {
            parts.push(r#"<span class="needs-human-badge">🙋 needs human</span>"#.to_string());
        }
        if let Some(ref p) = card.priority {
            let cls = match p.as_str() {
                "high" => "priority-badge priority-high",
                "medium" => "priority-badge priority-medium",
                _ => "priority-badge priority-low",
            };
            parts.push(format!(r#"<span class="{}">{}</span>"#, cls, escape_html(p)));
        }
        if let Some(due) = card.due_at {
            let today = chrono::Utc::now().date_naive();
            let due_date = due.date_naive();
            let cls = if due_date < today { "due-badge due-overdue" } else { "due-badge" };
            parts.push(format!(
                r#"<span class="{}">{}</span>"#,
                cls,
                due_date.format("%b %-d")
            ));
        }
        if let Some(owner) = &card.owner {
            parts.push(format!(
                r#"<span class="owner">{}</span>"#,
                escape_html(owner)
            ));
        }
        for tag in &card.tags {
            parts.push(format!(r#"<span class="tag">{}</span>"#, escape_html(tag)));
        }
        parts.join("")
    };

    let meta_div = if meta_html.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="card-meta">{}</div>"#, meta_html)
    };

    let checklist_html = render_card_checklist(card);

    let checklist_json = card
        .checklist
        .iter()
        .map(|i| {
            format!(
                r#"{{"text":{},"checked":{}}}"#,
                serde_json::to_string(&i.text).unwrap_or_default(),
                i.checked
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let extra_classes = format!(
        "{}{}",
        if card.agent { " agent-working" } else { "" },
        if card.needs_human { " needs-human" } else { "" }
    );

    format!(
        r#"<div class="card{extra_classes}"
  data-name="{name}"
  data-description="{desc}"
  data-status="{status}"
  data-owner="{owner}"
  data-tags="{tags}"
  data-due="{due}"
  data-priority="{priority}"
  data-needs-human="{needs_human}"
  data-checklist="[{checklist_json}]"
  onclick="openEdit(this)">
  <div class="card-name">{name_display}</div>
  {desc_display}{meta_div}{checklist_html}
</div>"#,
        extra_classes = extra_classes,
        needs_human = card.needs_human,
        name = escape_html(&card.name),
        desc = escape_html(&card.description),
        status = escape_html(&card.status),
        owner = escape_html(card.owner.as_deref().unwrap_or("")),
        tags = escape_html(&card.tags.join(",")),
        due = escape_html(
            &card.due_at
                .map(|d| d.date_naive().format("%Y-%m-%d").to_string())
                .unwrap_or_default()
        ),
        priority = escape_html(card.priority.as_deref().unwrap_or("")),
        checklist_json = escape_html(&checklist_json),
        name_display = escape_html(&card.name),
        desc_display = desc_display,
        meta_div = meta_div,
        checklist_html = checklist_html,
    )
}

fn render_card_checklist(card: &Card) -> String {
    if card.checklist.is_empty() {
        return String::new();
    }

    let total = card.checklist.len();
    let done = card.checklist.iter().filter(|i| i.checked).count();
    let pct = if total > 0 { done * 100 / total } else { 0 };

    format!(
        r#"<div class="checklist-progress">
  <div class="progress-bar"><div class="progress-fill" style="width:{pct}%"></div></div>
  <span class="progress-label">{done}/{total}</span>
</div>"#,
        pct = pct,
        done = done,
        total = total,
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
