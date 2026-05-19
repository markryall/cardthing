use crate::models::{Card, Status};
use crate::storage;
use anyhow::Result;
use axum::{extract::Path, response::Html, routing::get, Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

pub fn execute(port: u16) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(serve(port))
}

async fn serve(port: u16) -> Result<()> {
    let app = Router::new()
        .route("/", get(board_handler))
        .route("/cards/:name/status", axum::routing::patch(patch_card_status));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Cardthing board running at http://localhost:{}", port);
    println!("Press Ctrl-C to stop.");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
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

#[derive(Deserialize)]
struct StatusUpdate {
    status: String,
}

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn patch_card_status(
    Path(name): Path<String>,
    Json(body): Json<StatusUpdate>,
) -> Json<ApiResponse> {
    let result = (|| -> anyhow::Result<()> {
        let mut card = storage::load_card(&name)?;
        card.status = Status::from_str(&body.status)?;
        card.updated_at = Utc::now();
        storage::save_card(&card)?;
        Ok(())
    })();

    match result {
        Ok(_) => Json(ApiResponse { ok: true, error: None }),
        Err(e) => Json(ApiResponse { ok: false, error: Some(e.to_string()) }),
    }
}

struct Column {
    status: Status,
    status_str: &'static str,
    label: &'static str,
    color: &'static str,
}

fn columns() -> [Column; 4] {
    [
        Column { status: Status::Todo,       status_str: "todo",       label: "Todo",        color: "#f59e0b" },
        Column { status: Status::InProgress, status_str: "inprogress", label: "In Progress", color: "#3b82f6" },
        Column { status: Status::Done,       status_str: "done",       label: "Done",        color: "#10b981" },
        Column { status: Status::Blocked,    status_str: "blocked",    label: "Blocked",     color: "#ef4444" },
    ]
}

fn render_board(cards: &[Card]) -> String {
    let columns_html: String = columns()
        .iter()
        .map(|col| {
            let col_cards: Vec<&Card> = cards.iter().filter(|c| c.status == col.status).collect();
            render_column(col, &col_cards)
        })
        .collect();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Cardthing Board</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; }}
  header {{ padding: 1.5rem 2rem; border-bottom: 1px solid #1e293b; display: flex; align-items: center; gap: 0.75rem; }}
  header h1 {{ font-size: 1.25rem; font-weight: 600; letter-spacing: -0.01em; color: #f8fafc; }}
  header span {{ font-size: 0.8rem; color: #64748b; }}
  .board {{ display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; padding: 1.5rem 2rem; align-items: start; }}
  .column {{ background: #1e293b; border-radius: 0.75rem; padding: 1rem; }}
  .column-header {{ display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }}
  .column-label {{ font-size: 0.8rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; }}
  .column-count {{ font-size: 0.75rem; background: #0f172a; color: #94a3b8; border-radius: 999px; padding: 0.1rem 0.5rem; font-weight: 500; }}
  .column-cards {{ min-height: 2rem; }}
  .card {{ background: #0f172a; border-radius: 0.5rem; padding: 0.875rem; margin-bottom: 0.625rem; border: 1px solid #1e293b; cursor: grab; }}
  .card:last-child {{ margin-bottom: 0; }}
  .card:active {{ cursor: grabbing; }}
  .card-name {{ font-size: 0.875rem; font-weight: 600; color: #f1f5f9; margin-bottom: 0.375rem; }}
  .card-desc {{ font-size: 0.775rem; color: #94a3b8; line-height: 1.4; margin-bottom: 0.5rem; }}
  .card-meta {{ display: flex; flex-wrap: wrap; gap: 0.375rem; align-items: center; }}
  .owner {{ font-size: 0.7rem; color: #cbd5e1; background: #1e293b; border-radius: 999px; padding: 0.1rem 0.5rem; }}
  .tag {{ font-size: 0.7rem; color: #94a3b8; background: #0f172a; border: 1px solid #334155; border-radius: 999px; padding: 0.1rem 0.5rem; }}
  .empty {{ font-size: 0.775rem; color: #475569; text-align: center; padding: 1.5rem 0; }}
  .sortable-ghost {{ opacity: 0.3; }}
  .sortable-drag {{ opacity: 0.9; box-shadow: 0 8px 24px rgba(0,0,0,0.4); }}
  @media (max-width: 900px) {{ .board {{ grid-template-columns: repeat(2, 1fr); }} }}
  @media (max-width: 500px) {{ .board {{ grid-template-columns: 1fr; }} }}
</style>
</head>
<body>
<header>
  <h1>Cardthing Board</h1>
  <span>{total} cards</span>
</header>
<div class="board">
{columns}
</div>
<script src="https://cdn.jsdelivr.net/npm/sortablejs@1.15.6/Sortable.min.js"></script>
<script>
  document.querySelectorAll('.column').forEach(col => {{
    const cards = col.querySelector('.column-cards');
    new Sortable(cards, {{
      group: 'cards',
      animation: 150,
      ghostClass: 'sortable-ghost',
      dragClass: 'sortable-drag',
      onEnd(evt) {{
        if (evt.from === evt.to) return;
        const name = evt.item.dataset.name;
        const status = evt.to.closest('.column').dataset.status;
        fetch(`/cards/${{encodeURIComponent(name)}}/status`, {{
          method: 'PATCH',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{ status }}),
        }})
          .then(r => r.json())
          .then(data => {{ if (!data.ok) location.reload(); }})
          .catch(() => location.reload());
      }}
    }});
  }});
</script>
</body>
</html>"#,
        total = cards.len(),
        columns = columns_html,
    )
}

fn render_column(col: &Column, cards: &[&Card]) -> String {
    let cards_html: String = if cards.is_empty() {
        "<div class=\"empty\">No cards</div>".to_string()
    } else {
        cards.iter().map(|c| render_card(c)).collect()
    };

    format!(
        r#"<div class="column" data-status="{status}">
  <div class="column-header">
    <span class="column-label" style="color:{color}">{label}</span>
    <span class="column-count">{count}</span>
  </div>
  <div class="column-cards">
    {cards}
  </div>
</div>"#,
        status = col.status_str,
        color = col.color,
        label = col.label,
        count = cards.len(),
        cards = cards_html,
    )
}

fn render_card(card: &Card) -> String {
    let desc = if card.description.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="card-desc">{}</div>"#,
            escape_html(&truncate(&card.description, 120))
        )
    };

    let meta: String = {
        let mut parts = Vec::new();
        if let Some(owner) = &card.owner {
            parts.push(format!(
                r#"<span class="owner">{}</span>"#,
                escape_html(owner)
            ));
        }
        for tag in &card.tags {
            parts.push(format!(
                r#"<span class="tag">{}</span>"#,
                escape_html(tag)
            ));
        }
        parts.join("")
    };

    let meta_div = if meta.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="card-meta">{}</div>"#, meta)
    };

    format!(
        r#"<div class="card" data-name="{name_attr}">
  <div class="card-name">{name}</div>
  {desc}{meta}
</div>"#,
        name_attr = escape_html(&card.name),
        name = escape_html(&card.name),
        desc = desc,
        meta = meta_div,
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
