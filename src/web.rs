use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, Path, State};
use axum::response::{Html, Redirect};
use axum::routing::{get, post};
use chrono::Local;
use tokio::sync::Mutex;

use crate::card::{self, Card};
use crate::fsrs::Grade;
use crate::review;

// -- Static assets embedded at compile time --

const BASE_CSS: &str = include_str!("static/style.css");
const REVIEW_JS: &str = include_str!("static/review.js");

// -- App state --

struct AppState {
    cards: Vec<Card>,
    sources: Vec<PathBuf>,
}

struct ReviewSession {
    order: Vec<usize>,
    position: usize,
    counts: [u32; 4],
}

struct ServerState {
    app: AppState,
    sessions: HashMap<String, ReviewSession>,
}

type SharedState = Arc<Mutex<ServerState>>;

// -- HTML helpers --

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn page(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — rote</title>
<style>body{{background:#1e1e1e;color:#d4d4d4}}</style>
<script src="https://cdn.tailwindcss.com"></script>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.21/dist/katex.min.css">
<style>{base_css}</style>
</head>
<body class="bg-[#1e1e1e] text-[#d4d4d4] font-sans antialiased h-screen">
{body}
<script>{js}</script>
<script src="https://cdn.jsdelivr.net/npm/katex@0.16.21/dist/katex.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/katex@0.16.21/dist/contrib/auto-render.min.js"></script>
<script>document.addEventListener("DOMContentLoaded",function(){{renderMathInElement(document.body,{{delimiters:[{{left:"$$",right:"$$",display:true}},{{left:"$",right:"$",display:false}}],throwOnError:false}});}});</script>
</body>
</html>"#,
        title = html_escape(title),
        body = body,
        base_css = BASE_CSS,
        js = REVIEW_JS,
    )
}

fn sidebar_html(summaries: &[review::DeckSummary], active_deck: &str) -> String {
    let mut items = String::new();
    for s in summaries {
        let active = if s.name == active_deck {
            " bg-[#333] !text-[#e0e0e0]"
        } else {
            ""
        };
        let badge = if s.due > 0 {
            format!(
                r#"<span class="text-[0.7rem] bg-[#444] text-[#ccc] px-1.5 py-0.5 rounded-full min-w-5 text-center">{}</span>"#,
                s.due
            )
        } else {
            String::new()
        };
        items.push_str(&format!(
            r#"<li><a href="/deck/{name}" class="flex items-center justify-between px-4 py-1.5 text-[#999] text-sm no-underline hover:bg-[#2a2a2a] hover:!text-[#d4d4d4]{active}">{name}{badge}</a></li>"#,
            name = html_escape(&s.name),
        ));
    }
    format!(
        r#"<div class="w-56 shrink-0 bg-[#252525] border-r border-[#333] py-5 overflow-y-auto flex flex-col">
<div class="px-4 pb-4 text-[0.95rem] font-semibold text-[#e0e0e0]"><a href="/" class="!text-inherit no-underline">rote</a></div>
<div class="px-4 py-2 pb-1 text-[0.65rem] uppercase tracking-widest text-[#666]">Decks</div>
<nav><ul class="list-none m-0 p-0">{items}</ul></nav>
<div class="flex-1"></div>
</div>"#,
    )
}

fn breadcrumb(crumbs: &[(&str, &str)]) -> String {
    let mut parts = String::new();
    for (i, (label, href)) in crumbs.iter().enumerate() {
        if i > 0 {
            parts.push_str(r#"<span class="mx-1.5 text-[#555]">/</span>"#);
        }
        if href.is_empty() {
            parts.push_str(&html_escape(label));
        } else {
            parts.push_str(&format!(
                r#"<a href="{}" class="!text-[#888] no-underline hover:!text-[#bbb]">{}</a>"#,
                html_escape(href),
                html_escape(label),
            ));
        }
    }
    parts
}

fn btn_primary(href: &str, label: &str) -> String {
    format!(
        r#"<a href="{}" class="inline-flex items-center gap-1.5 px-3.5 py-2 rounded-md text-sm font-medium bg-[#4a90d9] !text-white no-underline hover:bg-[#5a9de6]">{}</a>"#,
        html_escape(href),
        label,
    )
}

fn btn_secondary(href: &str, label: &str) -> String {
    format!(
        r#"<a href="{}" class="inline-flex items-center gap-1.5 px-3.5 py-2 rounded-md text-sm font-medium bg-[#383838] !text-[#ccc] border border-[#444] no-underline hover:bg-[#444] hover:!text-[#e0e0e0]">{}</a>"#,
        html_escape(href),
        label,
    )
}

// -- Route handlers --

async fn index(State(state): State<SharedState>) -> Html<String> {
    let st = state.lock().await;
    let today = Local::now().date_naive();
    let summaries = review::deck_summaries(&st.app.cards, today);

    let sidebar = sidebar_html(&summaries, "");

    let total_due: usize = summaries.iter().map(|s| s.due).sum();
    let review_all = if total_due > 0 {
        btn_primary("/deck/_all/review", &format!("Review all {total_due} due"))
    } else {
        String::new()
    };

    let mut rows = String::new();
    if summaries.is_empty() {
        rows.push_str(r#"<p class="text-center text-[#666] py-12">No decks loaded.</p>"#);
    } else {
        rows.push_str(r#"<div class="flex flex-col gap-1">"#);
        for s in &summaries {
            let due_label = if s.due > 0 {
                format!(
                    r#"<span class="text-[#6ba3d6] font-medium">{} due</span>"#,
                    s.due
                )
            } else {
                String::new()
            };
            rows.push_str(&format!(
                r#"<a href="/deck/{name}" class="flex justify-between items-center py-2.5 px-3 bg-[#2a2a2a] rounded-md !text-[#d4d4d4] text-[0.9rem] no-underline hover:bg-[#333]">{name}<span class="flex items-center gap-3 text-sm text-[#888]">{total} cards{due}</span></a>"#,
                name = html_escape(&s.name),
                total = s.total,
                due = due_label,
            ));
        }
        rows.push_str("</div>");
    }

    let body = format!(
        r#"<div class="flex h-screen">
{sidebar}
<div class="flex-1 overflow-y-auto min-w-0">
<div class="flex items-center justify-between px-6 py-3 border-b border-[#333] bg-[#232323]">
<div class="text-sm text-[#888]">{bc}</div>
<div class="flex gap-2 items-center">{review_all}</div>
</div>
<div class="p-6 max-w-5xl">{rows}</div>
</div>
</div>"#,
        sidebar = sidebar,
        bc = breadcrumb(&[("Decks", "")]),
        review_all = review_all,
        rows = rows,
    );
    Html(page("Decks", &body))
}

async fn deck_detail(State(state): State<SharedState>, Path(name): Path<String>) -> Html<String> {
    let st = state.lock().await;
    let today = Local::now().date_naive();
    let summaries = review::deck_summaries(&st.app.cards, today);

    let sidebar = sidebar_html(&summaries, &name);

    let deck_cards: Vec<(usize, &Card)> = st
        .app
        .cards
        .iter()
        .enumerate()
        .filter(|(_, c)| c.deck == name)
        .collect();

    let due_count = deck_cards
        .iter()
        .filter(|(_, c)| c.due.is_none() || c.due.unwrap() <= today)
        .count();

    let mut header_actions = String::new();
    if due_count > 0 {
        header_actions.push_str(&btn_primary(
            &format!("/deck/{}/review", html_escape(&name)),
            &format!("Review {due_count} due"),
        ));
    }
    header_actions.push_str(&btn_secondary(
        &format!("/deck/{}/new", html_escape(&name)),
        "Add card",
    ));

    let mut tiles = String::new();
    for (_, c) in &deck_cards {
        let front_trunc = if c.front.len() > 80 {
            format!("{}…", &c.front[..80])
        } else {
            c.front.clone()
        };
        let back_trunc = if c.back.len() > 60 {
            format!("{}…", &c.back[..60])
        } else {
            c.back.clone()
        };
        let status = if c.due.is_none() {
            r#"<span class="text-[#888]">NEW</span>"#.to_string()
        } else if c.due.unwrap() <= today {
            r#"<span class="text-[#6ba3d6]">DUE</span>"#.to_string()
        } else {
            format!(
                r#"<span class="text-[#666]">{}</span>"#,
                c.due.unwrap().format("%b %d")
            )
        };
        tiles.push_str(&format!(
            r#"<a href="/card/{id}/edit" class="bg-[#2d2d2d] border border-[#3a3a3a] rounded-lg p-5 min-h-40 flex flex-col justify-between no-underline hover:border-[#555] transition-colors">
<div class="text-[0.9rem] font-medium text-[#e0e0e0] text-center flex-1 flex items-center justify-center overflow-hidden break-words">{front}</div>
<div class="text-xs text-[#888] text-center mt-3 overflow-hidden text-ellipsis whitespace-nowrap">{back}</div>
<div class="flex items-center gap-1 text-[0.65rem] mt-3 uppercase tracking-wider">{status}</div>
</a>"#,
            id = html_escape(&c.id),
            front = html_escape(&front_trunc),
            back = html_escape(&back_trunc),
            status = status,
        ));
    }

    // "Add card" tile
    tiles.push_str(&format!(
        r#"<a href="/deck/{name}/new" class="bg-transparent border border-dashed border-[#444] rounded-lg p-5 min-h-40 flex items-center justify-center text-[#666] text-sm no-underline cursor-pointer hover:border-[#666] hover:!text-[#999]">+ Add card</a>"#,
        name = html_escape(&name),
    ));

    let body = format!(
        r#"<div class="flex h-screen">
{sidebar}
<div class="flex-1 overflow-y-auto min-w-0">
<div class="flex items-center justify-between px-6 py-3 border-b border-[#333] bg-[#232323]">
<div class="text-sm text-[#888]">{bc}</div>
<div class="flex gap-2 items-center">{actions}</div>
</div>
<div class="p-6 max-w-5xl">
<div class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-4">{tiles}</div>
</div>
</div>
</div>
<script>document.addEventListener('keydown',function(e){{if(e.target.tagName==='INPUT'||e.target.tagName==='TEXTAREA')return;if(e.key==='r'){{var a=document.querySelector('[href*="/review"]');if(a)window.location=a.href;}}else if(e.key==='n'){{window.location='/deck/{name_enc}/new';}}}});</script>"#,
        sidebar = sidebar,
        bc = breadcrumb(&[("Decks", "/"), (&name, "")]),
        actions = header_actions,
        tiles = tiles,
        name_enc = html_escape(&name),
    );
    Html(page(&name, &body))
}

async fn review_page(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Form(params): Form<HashMap<String, String>>,
) -> axum::response::Response {
    let mut st = state.lock().await;
    let today = Local::now().date_naive();

    let session_id = params.get("session").cloned().unwrap_or_default();

    // If no valid session, create one
    if session_id.is_empty() || !st.sessions.contains_key(&session_id) {
        let due_indices: Vec<usize> = st
            .app
            .cards
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                (name == "_all" || c.deck == name) && (c.due.is_none() || c.due.unwrap() <= today)
            })
            .map(|(i, _)| i)
            .collect();

        if due_indices.is_empty() {
            let back = if name == "_all" {
                "/".to_string()
            } else {
                format!("/deck/{}", name)
            };
            return Redirect::to(&back).into_response();
        }

        let mut order = due_indices;
        shuffle(&mut order);

        let new_id = uuid::Uuid::new_v4().to_string();
        st.sessions.insert(
            new_id.clone(),
            ReviewSession {
                order,
                position: 0,
                counts: [0; 4],
            },
        );

        return Redirect::to(&format!("/deck/{}/review?session={}", name, new_id)).into_response();
    }

    let summaries = review::deck_summaries(&st.app.cards, today);
    let sidebar = sidebar_html(&summaries, &name);
    let session = st.sessions.get(&session_id).unwrap();

    if session.position >= session.order.len() {
        return Redirect::to(&format!("/deck/{}/summary?session={}", name, session_id))
            .into_response();
    }

    let card_idx = session.order[session.position];
    let card = &st.app.cards[card_idx];
    let front_display = review::render_front(&card.front);

    // Build back section HTML (hidden until reveal)
    let has_cloze = !card::extract_cloze_deletions(&card.front).is_empty();
    let back_text = card::expand_newlines(&card.back);
    let answer_cls =
        "px-8 py-10 text-center text-lg leading-relaxed text-[#e0e0e0] whitespace-pre-wrap";
    let back_html = match (has_cloze, back_text.trim().is_empty()) {
        (true, true) => {
            let filled = card::expand_newlines(&card.front.replace(['[', ']'], ""));
            format!(
                r#"<hr class="border-0 border-t border-dashed border-[#444] mx-8"><div class="{cls}">{text}</div>"#,
                cls = answer_cls,
                text = html_escape(&filled),
            )
        }
        (true, false) => {
            let filled = card::expand_newlines(&card.front.replace(['[', ']'], ""));
            format!(
                r#"<hr class="border-0 border-t border-dashed border-[#444] mx-8"><div class="{cls}">{top}</div><hr class="border-0 border-t border-dashed border-[#444] mx-8"><div class="{cls}">{bot}</div>"#,
                cls = answer_cls,
                top = html_escape(&filled),
                bot = html_escape(&back_text),
            )
        }
        (false, _) => {
            format!(
                r#"<hr class="border-0 border-t border-dashed border-[#444] mx-8"><div class="{cls}">{text}</div>"#,
                cls = answer_cls,
                text = html_escape(&back_text),
            )
        }
    };

    let position = session.position + 1;
    let total = session.order.len();

    let deck_display = if name == "_all" { "All decks" } else { &name };
    let deck_href = if name == "_all" {
        "/".to_string()
    } else {
        format!("/deck/{}", name)
    };

    let body = format!(
        r#"<div class="flex h-screen">
{sidebar}
<div class="flex-1 min-w-0 flex flex-col">
<div class="flex items-center justify-between px-6 py-3 border-b border-[#333] bg-[#232323]">
<div class="text-sm text-[#888]">{bc}</div>
<div class="flex items-center gap-1.5 text-sm text-[#888]">Card {pos} of {total}</div>
</div>
<div class="flex-1 flex items-center justify-center p-8">
<div class="w-full max-w-[620px]">
<div class="bg-[#2d2d2d] border border-[#3a3a3a] rounded-xl overflow-hidden">
<div class="{answer_cls}">{front}</div>
<div id="back-section" style="display:none">{back_html}</div>
<button type="button" id="reveal-btn" class="w-full py-3 text-[#888] text-sm text-center border-t border-[#333] cursor-pointer hover:bg-[#333] hover:!text-[#ccc]">Show Answer</button>
</div>
</div>
</div>
<div class="text-center py-2 text-sm text-[#666]" id="reveal-hint">Press <span class="inline-block px-1.5 py-0.5 text-xs bg-[#383838] border border-[#555] rounded text-[#aaa]">Space</span> to reveal</div>
<form id="grade-form" method="post" action="/deck/{name_enc}/review" style="display:none">
<input type="hidden" name="session" value="{session_id}">
<input type="hidden" name="grade" value="">
<div class="border-t border-[#333] bg-[#232323] px-6 py-3 flex items-center justify-center gap-4">
<button type="submit" onclick="this.form.grade.value='1'" class="inline-flex items-center gap-1 px-5 py-2 rounded-md text-sm font-medium cursor-pointer bg-[#333] text-[#e06c6c] hover:bg-[#3d2a2a]"><span class="inline-block px-1.5 py-0.5 text-xs bg-[#383838] border border-[#555] rounded text-[#aaa] mr-1">1</span> Forgot</button>
<button type="submit" onclick="this.form.grade.value='2'" class="inline-flex items-center gap-1 px-5 py-2 rounded-md text-sm font-medium cursor-pointer bg-[#333] text-[#d4a05a] hover:bg-[#3d3425]"><span class="inline-block px-1.5 py-0.5 text-xs bg-[#383838] border border-[#555] rounded text-[#aaa] mr-1">2</span> Hard</button>
<button type="submit" onclick="this.form.grade.value='3'" class="inline-flex items-center gap-1 px-5 py-2 rounded-md text-sm font-medium cursor-pointer bg-[#333] text-[#6bc06b] hover:bg-[#2a3d2a]"><span class="inline-block px-1.5 py-0.5 text-xs bg-[#383838] border border-[#555] rounded text-[#aaa] mr-1">3</span> Good</button>
<button type="submit" onclick="this.form.grade.value='4'" class="inline-flex items-center gap-1 px-5 py-2 rounded-md text-sm font-medium cursor-pointer bg-[#333] text-[#6ba3d6] hover:bg-[#2a2f3d]"><span class="inline-block px-1.5 py-0.5 text-xs bg-[#383838] border border-[#555] rounded text-[#aaa] mr-1">4</span> Easy</button>
</div>
</form>
</div>
</div>"#,
        sidebar = sidebar,
        bc = breadcrumb(&[("Decks", "/"), (deck_display, &deck_href), ("Review", "")]),
        pos = position,
        total = total,
        answer_cls = answer_cls,
        front = html_escape(&front_display),
        back_html = back_html,
        name_enc = html_escape(&name),
        session_id = html_escape(&session_id),
    );

    axum::response::Response::builder()
        .header("content-type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(page("Review", &body)))
        .unwrap()
}

async fn review_submit(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Form(params): Form<HashMap<String, String>>,
) -> Redirect {
    let mut st = state.lock().await;
    let session_id = params.get("session").cloned().unwrap_or_default();
    let grade_str = params.get("grade").cloned().unwrap_or_default();

    let grade = grade_str
        .parse::<u8>()
        .ok()
        .and_then(Grade::from_u8)
        .unwrap_or(Grade::Good);

    let session_info = st.sessions.get(&session_id).and_then(|session| {
        if session.position < session.order.len() {
            Some((session.order[session.position], session.position))
        } else {
            None
        }
    });

    if let Some((card_idx, _pos)) = session_info {
        let today = Local::now().date_naive();
        review::apply_grade(&mut st.app.cards[card_idx], grade, today);

        let source = st.app.sources[card_idx].clone();
        save_file(&st.app.cards, &st.app.sources, &source);

        let session = st.sessions.get_mut(&session_id).unwrap();
        let grade_idx = match grade {
            Grade::Forgot => 0,
            Grade::Hard => 1,
            Grade::Good => 2,
            Grade::Easy => 3,
        };
        session.counts[grade_idx] += 1;
        session.position += 1;
    }

    if let Some(session) = st.sessions.get(&session_id)
        && session.position >= session.order.len()
    {
        return Redirect::to(&format!("/deck/{}/summary?session={}", name, session_id));
    }

    Redirect::to(&format!("/deck/{}/review?session={}", name, session_id))
}

async fn review_get(
    state: State<SharedState>,
    path: Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> axum::response::Response {
    review_page(state, path, Form(params)).await
}

async fn summary_page(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Html<String> {
    let st = state.lock().await;
    let today = Local::now().date_naive();
    let summaries = review::deck_summaries(&st.app.cards, today);
    let sidebar = sidebar_html(&summaries, &name);
    let session_id = params.get("session").cloned().unwrap_or_default();

    let (counts, total) = if let Some(session) = st.sessions.get(&session_id) {
        let total: u32 = session.counts.iter().sum();
        (session.counts, total)
    } else {
        ([0u32; 4], 0)
    };

    let deck_display = if name == "_all" { "All decks" } else { &name };
    let deck_href = if name == "_all" {
        "/".to_string()
    } else {
        format!("/deck/{}", name)
    };
    let back_btn = if name == "_all" {
        btn_primary("/", "All decks")
    } else {
        btn_primary(&format!("/deck/{}", html_escape(&name)), "Back to deck")
    };

    let body = format!(
        r#"<div class="flex h-screen">
{sidebar}
<div class="flex-1 overflow-y-auto min-w-0">
<div class="flex items-center justify-between px-6 py-3 border-b border-[#333] bg-[#232323]">
<div class="text-sm text-[#888]">{bc}</div>
</div>
<div class="p-6 max-w-lg">
<h2 class="text-lg font-semibold text-[#e0e0e0] mb-4">Session Complete</h2>
<ul class="list-none m-0 mb-6 p-0">
<li class="flex justify-between py-2 border-b border-[#333] text-[0.9rem]"><span class="text-[#888]">Cards reviewed</span><span class="font-semibold text-[#e0e0e0]">{total}</span></li>
<li class="flex justify-between py-2 border-b border-[#333] text-[0.9rem]"><span class="text-[#e06c6c]">Forgot</span><span class="font-semibold text-[#e0e0e0]">{forgot}</span></li>
<li class="flex justify-between py-2 border-b border-[#333] text-[0.9rem]"><span class="text-[#d4a05a]">Hard</span><span class="font-semibold text-[#e0e0e0]">{hard}</span></li>
<li class="flex justify-between py-2 border-b border-[#333] text-[0.9rem]"><span class="text-[#6bc06b]">Good</span><span class="font-semibold text-[#e0e0e0]">{good}</span></li>
<li class="flex justify-between py-2 text-[0.9rem]"><span class="text-[#6ba3d6]">Easy</span><span class="font-semibold text-[#e0e0e0]">{easy}</span></li>
</ul>
<div class="flex gap-3">{back_btn}{home_btn}</div>
</div>
</div>
</div>"#,
        sidebar = sidebar,
        bc = breadcrumb(&[("Decks", "/"), (deck_display, &deck_href), ("Summary", ""),]),
        total = total,
        forgot = counts[0],
        hard = counts[1],
        good = counts[2],
        easy = counts[3],
        back_btn = back_btn,
        home_btn = btn_secondary("/", "Home"),
    );
    Html(page("Summary", &body))
}

async fn card_edit_form(State(state): State<SharedState>, Path(id): Path<String>) -> Html<String> {
    let st = state.lock().await;
    let today = Local::now().date_naive();
    let summaries = review::deck_summaries(&st.app.cards, today);
    let card = st.app.cards.iter().find(|c| c.id == id);

    let Some(card) = card else {
        return Html(page("Not Found", "<p>Card not found.</p>"));
    };

    let deck = card.deck.clone();
    let sidebar = sidebar_html(&summaries, &deck);

    let input_cls = "w-full px-3 py-2.5 border border-[#444] rounded-md text-[0.9rem] bg-[#383838] text-[#e0e0e0] focus:outline-none focus:border-[#6ba3d6] focus:ring-2 focus:ring-[#6ba3d6]/15";

    let body = format!(
        r#"<div class="flex h-screen">
{sidebar}
<div class="flex-1 overflow-y-auto min-w-0">
<div class="flex items-center justify-between px-6 py-3 border-b border-[#333] bg-[#232323]">
<div class="text-sm text-[#888]">{bc}</div>
</div>
<div class="p-6">
<div class="bg-[#2d2d2d] border border-[#3a3a3a] rounded-xl p-6 max-w-xl">
<div class="flex justify-between items-center mb-5">
<h2 class="text-lg font-semibold text-[#e0e0e0] m-0">Edit Card</h2>
<form method="post" action="/card/{id}/delete" onsubmit="return confirm('Delete this card?')" class="inline">
<button type="submit" class="inline-flex items-center gap-1 px-3.5 py-2 rounded-md text-sm font-medium bg-[#383838] text-[#e06c6c] border border-[#444] cursor-pointer hover:bg-[#3d2a2a]">Delete</button>
</form>
</div>
<form method="post" action="/card/{id}/edit">
<div class="mb-4">
<label class="block text-xs font-medium text-[#888] mb-1" for="deck">Deck</label>
<input type="text" id="deck" name="deck" value="{deck}" class="{input_cls}">
</div>
<div class="mb-4">
<label class="block text-xs font-medium text-[#888] mb-1" for="front">Front</label>
<textarea id="front" name="front" rows="4" class="{input_cls} min-h-[100px] resize-y leading-relaxed" style="font-family:inherit">{front}</textarea>
</div>
<div class="mb-4">
<label class="block text-xs font-medium text-[#888] mb-1" for="back">Back</label>
<textarea id="back" name="back" rows="4" class="{input_cls} min-h-[100px] resize-y leading-relaxed" style="font-family:inherit">{back}</textarea>
</div>
<div class="flex gap-3 mt-5">
<button type="submit" class="inline-flex items-center gap-1 px-3.5 py-2 rounded-md text-sm font-medium bg-[#4a90d9] text-white cursor-pointer hover:bg-[#5a9de6]">Save</button>
<a href="/deck/{deck_enc}" class="inline-flex items-center gap-1 px-3.5 py-2 rounded-md text-sm font-medium bg-[#383838] !text-[#ccc] border border-[#444] no-underline hover:bg-[#444] hover:!text-[#e0e0e0]">Cancel</a>
</div>
</form>
</div>
</div>
</div>
</div>
<script>document.addEventListener('keydown',function(e){{if((e.ctrlKey||e.metaKey)&&e.key==='Enter'){{e.preventDefault();document.querySelector('form[action*="edit"]').submit();}}}});</script>"#,
        sidebar = sidebar,
        bc = breadcrumb(&[
            ("Decks", "/"),
            (&deck, &format!("/deck/{}", deck)),
            ("Edit", ""),
        ]),
        id = html_escape(&id),
        deck = html_escape(&card.deck),
        deck_enc = html_escape(&card.deck),
        front = html_escape(&card.front),
        back = html_escape(&card.back),
        input_cls = input_cls,
    );
    Html(page("Edit Card", &body))
}

#[derive(serde::Deserialize)]
struct CardForm {
    deck: String,
    front: String,
    back: String,
}

async fn card_edit_submit(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Form(form): Form<CardForm>,
) -> Redirect {
    let mut st = state.lock().await;

    if let Some((i, card)) = st
        .app
        .cards
        .iter_mut()
        .enumerate()
        .find(|(_, c)| c.id == id)
    {
        card.deck = form.deck.clone();
        card.front = form.front;
        card.back = form.back;

        let source = st.app.sources[i].clone();
        save_file(&st.app.cards, &st.app.sources, &source);
    }

    Redirect::to(&format!("/deck/{}", form.deck))
}

async fn card_new_form(State(state): State<SharedState>, Path(name): Path<String>) -> Html<String> {
    let st = state.lock().await;
    let today = Local::now().date_naive();
    let summaries = review::deck_summaries(&st.app.cards, today);
    let sidebar = sidebar_html(&summaries, &name);

    let input_cls = "w-full px-3 py-2.5 border border-[#444] rounded-md text-[0.9rem] bg-[#383838] text-[#e0e0e0] focus:outline-none focus:border-[#6ba3d6] focus:ring-2 focus:ring-[#6ba3d6]/15";

    let body = format!(
        r#"<div class="flex h-screen">
{sidebar}
<div class="flex-1 overflow-y-auto min-w-0">
<div class="flex items-center justify-between px-6 py-3 border-b border-[#333] bg-[#232323]">
<div class="text-sm text-[#888]">{bc}</div>
</div>
<div class="p-6">
<div class="bg-[#2d2d2d] border border-[#3a3a3a] rounded-xl p-6 max-w-xl">
<h2 class="text-lg font-semibold text-[#e0e0e0] mb-5">New Card</h2>
<form method="post" action="/deck/{name_enc}/new">
<div class="mb-4">
<label class="block text-xs font-medium text-[#888] mb-1" for="front">Front</label>
<textarea id="front" name="front" rows="4" autofocus class="{input_cls} min-h-[100px] resize-y leading-relaxed" style="font-family:inherit"></textarea>
</div>
<div class="mb-4">
<label class="block text-xs font-medium text-[#888] mb-1" for="back">Back</label>
<textarea id="back" name="back" rows="4" class="{input_cls} min-h-[100px] resize-y leading-relaxed" style="font-family:inherit"></textarea>
</div>
<div class="flex gap-3 mt-5">
<button type="submit" class="inline-flex items-center gap-1 px-3.5 py-2 rounded-md text-sm font-medium bg-[#4a90d9] text-white cursor-pointer hover:bg-[#5a9de6]">Create</button>
<a href="/deck/{name_enc}" class="inline-flex items-center gap-1 px-3.5 py-2 rounded-md text-sm font-medium bg-[#383838] !text-[#ccc] border border-[#444] no-underline hover:bg-[#444] hover:!text-[#e0e0e0]">Cancel</a>
</div>
</form>
</div>
</div>
</div>
</div>
<script>document.addEventListener('keydown',function(e){{if((e.ctrlKey||e.metaKey)&&e.key==='Enter'){{e.preventDefault();document.querySelector('form').submit();}}}});</script>"#,
        sidebar = sidebar,
        bc = breadcrumb(&[
            ("Decks", "/"),
            (&name, &format!("/deck/{}", name)),
            ("New", ""),
        ]),
        name_enc = html_escape(&name),
        input_cls = input_cls,
    );
    Html(page("New Card", &body))
}

#[derive(serde::Deserialize)]
struct NewCardForm {
    front: String,
    back: String,
}

async fn card_new_submit(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Form(form): Form<NewCardForm>,
) -> Redirect {
    let mut st = state.lock().await;

    let source = st
        .app
        .cards
        .iter()
        .enumerate()
        .find(|(_, c)| c.deck == name)
        .map(|(i, _)| st.app.sources[i].clone())
        .or_else(|| st.app.sources.first().cloned());

    let Some(source) = source else {
        return Redirect::to("/");
    };

    let new_card = Card {
        deck: name.clone(),
        front: form.front,
        back: form.back,
        media: String::new(),
        id: uuid::Uuid::new_v4().to_string(),
        stability: None,
        difficulty: None,
        due: None,
        last_review: None,
    };

    st.app.sources.push(source.clone());
    st.app.cards.push(new_card);

    save_file(&st.app.cards, &st.app.sources, &source);

    Redirect::to(&format!("/deck/{}", name))
}

async fn card_delete(State(state): State<SharedState>, Path(id): Path<String>) -> Redirect {
    let mut st = state.lock().await;

    let pos = st.app.cards.iter().position(|c| c.id == id);
    if let Some(i) = pos {
        let deck = st.app.cards[i].deck.clone();
        let source = st.app.sources[i].clone();
        st.app.cards.remove(i);
        st.app.sources.remove(i);
        save_file(&st.app.cards, &st.app.sources, &source);
        return Redirect::to(&format!("/deck/{}", deck));
    }

    Redirect::to("/")
}

// -- Helpers --

fn save_file(cards: &[Card], sources: &[PathBuf], target: &PathBuf) {
    let file_cards: Vec<Card> = cards
        .iter()
        .enumerate()
        .filter(|(i, _)| sources[*i] == *target)
        .map(|(_, c)| c.clone())
        .collect();
    if let Err(e) = card::save_csv(target, &file_cards) {
        eprintln!("Error saving {}: {e}", target.display());
    }
}

fn shuffle<T>(items: &mut [T]) {
    let mut state: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    if state == 0 {
        state = 1;
    }
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        items.swap(i, j);
    }
}

use axum::response::IntoResponse;

// -- Public entry point --

pub async fn serve(paths: Vec<String>, port: u16) {
    let files = card::discover_files(&paths);
    if files.is_empty() {
        eprintln!("No CSV files found.");
        std::process::exit(1);
    }

    let mut all_cards: Vec<Card> = Vec::new();
    let mut card_sources: Vec<PathBuf> = Vec::new();

    for file in &files {
        match card::load_csv(file) {
            Ok(cards) => {
                for c in cards {
                    card_sources.push(file.clone());
                    all_cards.push(c);
                }
            }
            Err(e) => {
                eprintln!("Warning: {e}");
            }
        }
    }

    println!(
        "Loaded {} cards from {} files.",
        all_cards.len(),
        files.len()
    );

    let state = Arc::new(Mutex::new(ServerState {
        app: AppState {
            cards: all_cards,
            sources: card_sources,
        },
        sessions: HashMap::new(),
    }));

    let app = Router::new()
        .route("/", get(index))
        .route("/deck/{name}", get(deck_detail))
        .route("/deck/{name}/review", get(review_get).post(review_submit))
        .route("/deck/{name}/summary", get(summary_page))
        .route("/deck/{name}/new", get(card_new_form).post(card_new_submit))
        .route(
            "/card/{id}/edit",
            get(card_edit_form).post(card_edit_submit),
        )
        .route("/card/{id}/delete", post(card_delete))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    println!("Serving at http://localhost:{port}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
