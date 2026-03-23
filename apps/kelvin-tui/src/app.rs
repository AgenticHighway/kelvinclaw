use std::time::{Duration, Instant};

use crossterm::{
    cursor::SetCursorStyle,
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
            Event, EventStream, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind},
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, layout::Rect, widgets::TableState, Terminal};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::interval;

use unicode_width::UnicodeWidthChar;

use crate::{
    commands::{CompletionItem, LocalCommand, MergedCommandRegistry, SlashCommand},
    ui,
    ws_client::WsClient,
    CliConfig,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentEvent {
    pub seq: u64,
    pub data: AgentEventData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "stream", rename_all = "snake_case")]
pub enum AgentEventData {
    Lifecycle {
        run_id: String,
        phase: LifecyclePhase,
        message: Option<String>,
        ts_ms: u64,
    },
    Assistant {
        run_id: String,
        delta: String,
        final_chunk: bool,
        ts_ms: u64,
    },
    Tool {
        run_id: String,
        tool_name: String,
        phase: ToolPhase,
        summary: Option<String>,
        output: Option<String>,
        ts_ms: u64,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    Start,
    End,
    Error,
    Warning,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolPhase {
    Start,
    End,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

pub enum TuiEvent {
    Key(KeyEvent),
    Paste(String),
    Agent(AgentEvent),
    WsStatus(WsStatus),
    #[allow(dead_code)]
    Resize(u16, u16),
    Mouse(crossterm::event::MouseEvent),
    SubmitResult(Result<String, String>),
    Reconnected(Result<WsClient, String>),
    CommandsLoaded(Result<serde_json::Value, String>),
    CommandResult(Result<serde_json::Value, String>),
    Tick,
}

#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    Assistant { text: String, complete: bool },
    System(String),
}

#[derive(Debug, Clone)]
pub struct ToolEntry {
    pub tool_name: String,
    pub phase: ToolPhase,
    pub summary: Option<String>,
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct PasteMarker {
    pub start: usize,
    pub end: usize,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionTarget { Chat, Tools }

/// A position in the flat Vec<Line> built by chat::render.
/// `col` is a display-column offset within the full content line (including prefix spans).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatPos { pub line_idx: usize, pub col: usize }

#[derive(Debug, Clone)]
pub struct Selection {
    pub target: SelectionTarget,
    pub anchor: ChatPos,
    pub extent: ChatPos,
}

impl Selection {
    pub fn normalized(&self) -> (ChatPos, ChatPos) {
        let (a, b) = (self.anchor, self.extent);
        if a.line_idx < b.line_idx || (a.line_idx == b.line_idx && a.col <= b.col) {
            (a, b)
        } else {
            (b, a)
        }
    }
}

/// Per-content-line metadata, rebuilt each frame by chat::render.
#[derive(Debug, Clone, Default)]
pub struct ChatLineInfo {
    pub prefix_width: usize,
    pub is_separator: bool,
}

const PASTE_THRESHOLD_LINES: usize = 3;
const PASTE_THRESHOLD_BYTES: usize = 200;

fn is_large_paste(text: &str) -> bool {
    let lines = text.lines().count();
    lines >= PASTE_THRESHOLD_LINES || text.len() >= PASTE_THRESHOLD_BYTES
}

fn paste_label(text: &str) -> String {
    let lines = text.lines().count();
    let bytes = text.len();
    let size = if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    };
    format!("[pasted {lines} lines · {size}]")
}

pub fn build_display(input: &str, markers: &[PasteMarker]) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    for m in markers {
        out.push_str(&input[i..m.start]);
        out.push_str(&m.label);
        i = m.end;
    }
    out.push_str(&input[i..]);
    out
}

pub fn actual_to_display(pos: usize, markers: &[PasteMarker]) -> usize {
    let mut disp = 0;
    let mut i = 0;
    for m in markers {
        if pos <= m.start {
            return disp + pos.saturating_sub(i);
        }
        disp += m.start.saturating_sub(i);
        if pos < m.end {
            return disp + m.label.len();
        }
        disp += m.label.len();
        i = m.end;
    }
    disp + pos.saturating_sub(i)
}

pub fn display_to_actual(disp_pos: usize, markers: &[PasteMarker]) -> usize {
    let mut disp = 0;
    let mut i = 0;
    for m in markers {
        let before = m.start.saturating_sub(i);
        if disp_pos <= disp + before {
            return i + disp_pos.saturating_sub(disp);
        }
        disp += before;
        let llen = m.label.len();
        if disp_pos <= disp + llen {
            return m.end;
        }
        disp += llen;
        i = m.end;
    }
    i + disp_pos.saturating_sub(disp)
}

pub struct App {
    pub chat: Vec<ChatMessage>,
    pub tools: Vec<ToolEntry>,
    pub input: String,
    pub cursor_pos: usize,
    pub paste_markers: Vec<PasteMarker>,
    pub ws_status: WsStatus,
    pub run_phase: Option<LifecyclePhase>,
    pub current_run_id: Option<String>,
    pub gateway_url: String,
    pub auth_token: Option<String>,
    pub reconnecting: bool,
    pub last_error: Option<String>,
    pub chat_scroll: usize,
    pub chat_pinned: bool,
    pub chat_max_scroll: usize,
    pub tools_visible: bool,
    pub tools_scroll: usize,
    pub tools_pinned: bool,
    pub tools_max_scroll: usize,
    pub chat_area: Rect,
    pub tools_area: Rect,
    pub tools_table_state: TableState,
    pub should_quit: bool,
    pub last_ctrl_c: Option<Instant>,
    pub input_history: Vec<String>,
    pub history_idx: Option<usize>,
    pub history_saved: String,
    pub input_inner_width: usize, // updated each frame
    // Selection (chat_line_* rebuilt each frame by chat::render)
    pub selection: Option<Selection>,
    pub chat_line_map: Vec<(usize, usize)>, // visual_row -> (content_line_idx, char_start_col)
    pub chat_line_info: Vec<ChatLineInfo>,
    pub chat_line_texts: Vec<String>,       // per content line, prefix stripped (empty for separators)
    pub command_registry: MergedCommandRegistry,
    pub autocomplete_visible: bool,
    pub autocomplete_items: Vec<CompletionItem>,
    pub autocomplete_selected: usize,
}

impl App {
    pub fn new(gateway_url: String) -> Self {
        Self {
            chat: vec![ChatMessage::System("Connecting to gateway…".to_string())],
            tools: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            paste_markers: Vec::new(),
            ws_status: WsStatus::Connecting,
            run_phase: None,
            current_run_id: None,
            gateway_url,
            auth_token: None,
            reconnecting: false,
            last_error: None,
            chat_scroll: 0,
            chat_pinned: true,
            chat_max_scroll: 0,
            tools_visible: true,
            tools_scroll: 0,
            tools_pinned: true,
            tools_max_scroll: 0,
            chat_area: Rect::default(),
            tools_area: Rect::default(),
            tools_table_state: TableState::default(),
            should_quit: false,
            last_ctrl_c: None,
            input_history: Vec::new(),
            history_idx: None,
            history_saved: String::new(),
            input_inner_width: 0,
            selection: None,
            chat_line_map: Vec::new(),
            chat_line_info: Vec::new(),
            chat_line_texts: Vec::new(),
            command_registry: MergedCommandRegistry::default(),
            autocomplete_visible: false,
            autocomplete_items: Vec::new(),
            autocomplete_selected: 0,
        }
    }

    pub fn display_input(&self) -> String {
        build_display(&self.input, &self.paste_markers)
    }

    pub fn display_cursor(&self) -> usize {
        actual_to_display(self.cursor_pos, &self.paste_markers)
    }

    pub fn do_paste(&mut self, text: String) {
        self.paste_markers.retain(|m| m.end <= self.cursor_pos || m.start >= self.cursor_pos);

        let start = self.cursor_pos;
        let len = text.len();

        if is_large_paste(&text) {
            let label = paste_label(&text);
            self.input.insert_str(start, &text);
            for m in &mut self.paste_markers {
                if m.start >= start {
                    m.start += len;
                    m.end += len;
                }
            }
            let idx = self.paste_markers.partition_point(|m| m.start < start);
            self.paste_markers.insert(idx, PasteMarker { start, end: start + len, label });
        } else {
            self.input.insert_str(start, &text);
            for m in &mut self.paste_markers {
                if m.start >= start {
                    m.start += len;
                    m.end += len;
                }
            }
        }

        self.cursor_pos = start + len;
        #[cfg(debug_assertions)]
        assert_markers_valid(&self.input, &self.paste_markers);
    }

    pub fn do_delete(&mut self) {
        if self.cursor_pos >= self.input.len() {
            return;
        }
        if let Some(idx) = self.paste_markers.iter().position(|m| m.start == self.cursor_pos) {
            let m = self.paste_markers.remove(idx);
            let removed = m.end - m.start;
            self.input.drain(m.start..m.end);
            for later in &mut self.paste_markers {
                if later.start >= m.end {
                    later.start -= removed;
                    later.end -= removed;
                }
            }
        } else {
            let mut next = self.cursor_pos + 1;
            while next < self.input.len() && !self.input.is_char_boundary(next) {
                next += 1;
            }
            let removed_len = next - self.cursor_pos;
            self.input.drain(self.cursor_pos..next);
            for m in &mut self.paste_markers {
                if m.start > self.cursor_pos {
                    m.start -= removed_len;
                    m.end -= removed_len;
                }
            }
        }
        #[cfg(debug_assertions)]
        assert_markers_valid(&self.input, &self.paste_markers);
    }

    pub fn do_backspace(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        if let Some(idx) = self.paste_markers.iter().position(|m| m.end == self.cursor_pos) {
            let m = self.paste_markers.remove(idx);
            let removed = m.end - m.start;
            self.input.drain(m.start..m.end);
            self.cursor_pos = m.start;
            for later in &mut self.paste_markers {
                if later.start >= m.end {
                    later.start -= removed;
                    later.end -= removed;
                }
            }
        } else {
            let mut prev = self.cursor_pos - 1;
            while prev > 0 && !self.input.is_char_boundary(prev) {
                prev -= 1;
            }
            let removed_len = self.cursor_pos - prev;
            self.input.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
            for m in &mut self.paste_markers {
                if m.start > prev {
                    m.start -= removed_len;
                    m.end -= removed_len;
                }
            }
        }
        #[cfg(debug_assertions)]
        assert_markers_valid(&self.input, &self.paste_markers);
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos == 0 { return; }
        if let Some(m) = self.paste_markers.iter().find(|m| m.end == self.cursor_pos) {
            self.cursor_pos = m.start;
        } else {
            let mut new_pos = self.cursor_pos - 1;
            while new_pos > 0 && !self.input.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.cursor_pos = new_pos;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos >= self.input.len() { return; }
        if let Some(m) = self.paste_markers.iter().find(|m| m.start == self.cursor_pos) {
            self.cursor_pos = m.end;
        } else {
            let mut new_pos = self.cursor_pos + 1;
            while new_pos < self.input.len() && !self.input.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            self.cursor_pos = new_pos;
        }
    }

    fn compact_chat(&mut self) {
        const MAX_CHAT_MESSAGES: usize = 1000;
        if self.chat.len() <= MAX_CHAT_MESSAGES { return; }
        let to_remove = self.chat.len() - MAX_CHAT_MESSAGES;
        let prior_count = match self.chat.first() {
            Some(ChatMessage::System(s)) if s.starts_with('[') && s.contains("earlier messages omitted") => {
                s.trim_start_matches('[')
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0)
            }
            _ => 0,
        };
        self.chat.drain(0..to_remove);
        let total = to_remove + prior_count;
        if prior_count > 0 {
            self.chat[0] = ChatMessage::System(format!("[{total} earlier messages omitted]"));
        } else {
            self.chat.insert(0, ChatMessage::System(format!("[{total} earlier messages omitted]")));
        }
        self.selection = None; // line indices shifted
    }

    pub fn handle_agent_event(&mut self, ev: AgentEvent) {
        match ev.data {
            AgentEventData::Assistant { delta, final_chunk, .. } => {
                if let Some(ChatMessage::Assistant { text, complete }) = self.chat.last_mut() {
                    if !*complete {
                        text.push_str(&delta);
                        if final_chunk {
                            *complete = true;
                        }
                        return;
                    }
                }
                self.chat.push(ChatMessage::Assistant {
                    text: delta,
                    complete: final_chunk,
                });
                self.chat_pinned = true;
            }
            AgentEventData::Tool { tool_name, phase, summary, ts_ms, .. } => {
                if let ToolPhase::Start = phase {
                    if let Some(ChatMessage::Assistant { complete, .. }) = self.chat.last_mut() {
                        *complete = true;
                    }
                    self.tools.push(ToolEntry {
                        tool_name: tool_name.clone(),
                        phase: ToolPhase::Start,
                        summary: summary.clone(),
                        started_ms: ts_ms,
                        ended_ms: None,
                    });
                } else {
                    if let Some(entry) = self.tools.iter_mut().rev()
                        .find(|t| t.tool_name == tool_name && matches!(t.phase, ToolPhase::Start))
                    {
                        entry.phase = phase;
                        entry.summary = summary.or(entry.summary.clone());
                        entry.ended_ms = Some(ts_ms);
                    } else {
                        self.tools.push(ToolEntry {
                            tool_name,
                            phase,
                            summary,
                            started_ms: ts_ms,
                            ended_ms: Some(ts_ms),
                        });
                    }
                }
            }
            AgentEventData::Lifecycle { run_id, phase, message, .. } => {
                match &phase {
                    LifecyclePhase::Start => {
                        self.current_run_id = Some(run_id);
                        self.run_phase = Some(LifecyclePhase::Start);
                    }
                    LifecyclePhase::End => {
                        self.run_phase = Some(LifecyclePhase::End);
                    }
                    LifecyclePhase::Error => {
                        self.run_phase = Some(LifecyclePhase::Error);
                        self.last_error = message.clone();
                    }
                    LifecyclePhase::Warning => {}
                }
                if let Some(msg) = message {
                    if !msg.is_empty() {
                        self.chat.push(ChatMessage::System(msg));
                    }
                }
            }
        }
    }
}

fn in_rect(col: u16, row: u16, r: Rect) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

fn screen_to_chat_pos(col: u16, row: u16, app: &App) -> Option<ChatPos> {
    let inner_x = app.chat_area.x + 1;
    let inner_y = app.chat_area.y + 1;
    if col < inner_x || row < inner_y { return None; }
    let rel_col = (col - inner_x) as usize;
    let rel_row = (row - inner_y) as usize;
    let scroll = if app.chat_pinned { app.chat_max_scroll } else { app.chat_scroll.min(app.chat_max_scroll) };
    let abs_row = rel_row + scroll;
    let &(line_idx, char_start_col) = app.chat_line_map.get(abs_row)?;
    Some(ChatPos { line_idx, col: char_start_col + rel_col })
}

fn screen_to_tools_row(row: u16, app: &App) -> Option<usize> {
    let header_row = app.tools_area.y + 2; // top border + header row
    if row < header_row { return None; }
    let rel_row = (row - header_row) as usize;
    let offset = if app.tools_pinned { app.tools_max_scroll } else { app.tools_scroll.min(app.tools_max_scroll) };
    let idx = rel_row + offset;
    if idx < app.tools.len() { Some(idx) } else { None }
}

fn extract_selected_text(app: &App) -> String {
    match &app.selection {
        None => String::new(),
        Some(sel) if sel.target == SelectionTarget::Tools => {
            let idx = sel.anchor.line_idx;
            if let Some(e) = app.tools.get(idx) {
                let phase = match e.phase { ToolPhase::Start => "running", ToolPhase::End => "done", ToolPhase::Error => "error" };
                let summary = e.summary.as_deref().unwrap_or("");
                let dur = e.ended_ms.map_or("...".into(), |end| format!("{}ms", end.saturating_sub(e.started_ms)));
                format!("{}\t{}\t{}\t{}", e.tool_name, phase, summary, dur)
            } else { String::new() }
        }
        Some(sel) => {
            let (start, end) = sel.normalized();
            let n = app.chat_line_texts.len();
            if start.line_idx >= n { return String::new(); }
            // If the drag ended at column 0, the cursor is before that line — exclude it.
            let end_idx = if end.col == 0 && end.line_idx > start.line_idx {
                end.line_idx - 1
            } else {
                end.line_idx.min(n.saturating_sub(1))
            };
            let mut result = String::new();
            for idx in start.line_idx..=end_idx {
                let info = &app.chat_line_info[idx];
                if info.is_separator { continue; }
                let clean = &app.chat_line_texts[idx];
                let sel_start = if idx == start.line_idx { start.col } else { 0 };
                let sel_end = if idx == end_idx { end.col } else { usize::MAX };
                let text_start = sel_start.saturating_sub(info.prefix_width);
                let text_end = if sel_end == usize::MAX {
                    clean.len()
                } else {
                    let t = sel_end.saturating_sub(info.prefix_width);
                    // Selection lands within the prefix — include the full line.
                    if t == 0 && sel_end > 0 { clean.len() } else { t }
                };
                let byte_start = display_col_to_byte(clean, text_start);
                let byte_end = display_col_to_byte(clean, text_end).max(byte_start).min(clean.len());
                let slice = &clean[byte_start..byte_end];
                if !result.is_empty() { result.push('\n'); }
                result.push_str(slice);
            }
            result
        }
    }
}

fn display_col_to_byte(s: &str, col: usize) -> usize {
    let mut width = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if width >= col { return byte_idx; }
        width += ch.width().unwrap_or(0);
    }
    s.len()
}

fn copy_osc52(text: &str) {
    use std::io::Write;
    let encoded = base64_encode(text.as_bytes());
    let _ = write!(std::io::stdout(), "\x1b]52;c;{}\x07", encoded);
    let _ = std::io::stdout().flush();
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 0x3f) as usize] as char);
        out.push(T[((n >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 0x3f) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 0x3f) as usize] as char } else { '=' });
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Update the autocomplete popup based on the current input.
fn update_autocomplete(app: &mut App) {
    // Only trigger if input starts with '/' and cursor is still in the command name
    // (i.e., no space has been typed yet after the command name).
    if app.input.starts_with('/') {
        let after_slash = &app.input[1..];
        if !after_slash.contains(' ') {
            let items = app.command_registry.completions(after_slash);
            app.autocomplete_visible = !items.is_empty();
            // Clamp selected index.
            if app.autocomplete_selected >= items.len() {
                app.autocomplete_selected = 0;
            }
            app.autocomplete_items = items;
            return;
        }
    }
    app.autocomplete_visible = false;
    app.autocomplete_items.clear();
    app.autocomplete_selected = 0;
}

/// Format a command result payload into a readable string for the chat.
fn format_command_result(payload: &serde_json::Value) -> String {
    let command = payload.get("command").and_then(|v| v.as_str()).unwrap_or("?");
    match command {
        "tools" => {
            let count = payload.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            let mut lines = vec![format!("Tools ({count}):") ];
            if let Some(tools) = payload.get("tools").and_then(|v| v.as_array()) {
                for tool in tools {
                    let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let desc = tool.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    lines.push(format!("  {name} — {desc}"));
                }
            }
            lines.join("\n")
        }
        "sessions" | "plugins" => {
            serde_json::to_string_pretty(payload).unwrap_or_else(|_| format!("{payload}"))
        }
        _ => serde_json::to_string_pretty(payload).unwrap_or_else(|_| format!("{payload}")),
    }
}

fn word_left(s: &str, pos: usize) -> usize {
    let mut current = pos;
    for c in s[..current].chars().rev() {
        if is_word_char(c) { break; }
        current -= c.len_utf8();
    }
    for c in s[..current].chars().rev() {
        if !is_word_char(c) { break; }
        current -= c.len_utf8();
    }
    current
}

fn word_right(s: &str, pos: usize) -> usize {
    let mut current = pos;
    for c in s[current..].chars() {
        if is_word_char(c) { break; }
        current += c.len_utf8();
    }
    for c in s[current..].chars() {
        if !is_word_char(c) { break; }
        current += c.len_utf8();
    }
    current
}

#[cfg(debug_assertions)]
fn assert_markers_valid(input: &str, markers: &[PasteMarker]) {
    for i in 0..markers.len() {
        assert!(markers[i].start < markers[i].end, "marker {i} start >= end");
        assert!(markers[i].end <= input.len(), "marker {i} end out of bounds");
        if i + 1 < markers.len() {
            assert!(
                markers[i].end <= markers[i + 1].start,
                "markers {i} and {} overlap",
                i + 1
            );
        }
    }
}

fn visual_line_start(display_pos: usize, inner_width: usize) -> usize {
    if inner_width < 3 { return 0; }
    let prefix = 2;
    let first_cap = inner_width - prefix;
    if display_pos <= first_cap {
        0
    } else {
        let rest = display_pos - first_cap;
        first_cap + (rest / inner_width) * inner_width
    }
}

fn visual_line_end(display_pos: usize, display_len: usize, inner_width: usize) -> usize {
    if inner_width < 3 { return display_len; }
    let prefix = 2;
    let first_cap = inner_width - prefix;
    let end = if display_pos < first_cap {
        first_cap
    } else {
        let rest = display_pos - first_cap;
        first_cap + ((rest / inner_width) + 1) * inner_width
    };
    end.min(display_len)
}

// cleanup runs even on panic
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if let Err(e) = crossterm::terminal::disable_raw_mode() {
            eprintln!("warn: disable_raw_mode failed: {e}");
        }
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableBracketedPaste,
            DisableMouseCapture,
            crossterm::cursor::Show,
        ) {
            eprintln!("warn: terminal restore failed: {e}");
        }
    }
}

pub async fn run(config: CliConfig) -> Result<(), String> {
    let (tui_tx, mut tui_rx) = mpsc::channel::<TuiEvent>(256);

    enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, SetCursorStyle::SteadyBlock)
        .map_err(|e| format!("enter alt screen: {e}"))?;
    // best-effort: mouse scroll still works without this on some terminals
    let _ = execute!(stdout, EnableMouseCapture);

    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| format!("terminal: {e}"))?;

    let mut app = App::new(config.gateway_url.clone());
    app.auth_token = config.auth_token.clone();

    let ws_result = WsClient::connect(&config.gateway_url, config.auth_token.clone(), tui_tx.clone()).await;
    let ws_client = match ws_result {
        Ok(client) => {
            app.ws_status = WsStatus::Connected;
            app.chat.clear();
            app.chat.push(ChatMessage::System(format!("Connected to {}", config.gateway_url)));
            // Fetch available commands from the gateway.
            let fetch_client = client.clone();
            let tx = tui_tx.clone();
            tokio::spawn(async move {
                let result = fetch_client.list_commands().await;
                let _ = tx.send(TuiEvent::CommandsLoaded(result)).await;
            });
            Some(client)
        }
        Err(e) => {
            app.ws_status = WsStatus::Error(e.clone());
            app.last_error = Some(e.clone());
            app.chat.push(ChatMessage::System(format!("Connection failed: {e}")));
            None
        }
    };

    let tui_tx_key = tui_tx.clone();
    let key_task = tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(Ok(event)) = reader.next().await {
            match event {
                Event::Key(key) => { let _ = tui_tx_key.send(TuiEvent::Key(key)).await; }
                Event::Paste(text) => { let _ = tui_tx_key.send(TuiEvent::Paste(text)).await; }
                Event::Resize(w, h) => { let _ = tui_tx_key.send(TuiEvent::Resize(w, h)).await; }
                Event::Mouse(e) => { let _ = tui_tx_key.send(TuiEvent::Mouse(e)).await; }
                _ => {}
            }
        }
    });

    let tui_tx_tick = tui_tx.clone();
    let tick_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(100));
        loop {
            ticker.tick().await;
            if tui_tx_tick.send(TuiEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    let result = run_loop(&mut terminal, &mut app, &mut tui_rx, tui_tx.clone(), ws_client, &config.session_id).await;

    key_task.abort();
    tick_task.abort();
    drop(terminal);

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    tui_rx: &mut mpsc::Receiver<TuiEvent>,
    tui_tx: mpsc::Sender<TuiEvent>,
    ws_client: Option<WsClient>,
    session_id: &str,
) -> Result<(), String> {
    let mut ws_client = ws_client;
    loop {
        let frame = terminal.draw(|f| ui::render(f, app)).map_err(|e| format!("draw: {e}"))?;
        app.input_inner_width = frame.area.width.saturating_sub(2) as usize;

        let event = tui_rx.recv().await.ok_or("event channel closed")?;

        match event {
            TuiEvent::Tick => {}

            TuiEvent::Paste(text) => {
                app.do_paste(text);
            }

            TuiEvent::Key(key) => {
                match key.code {
                    KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.tools_visible = !app.tools_visible;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if app.selection.is_some() {
                            let text = extract_selected_text(app);
                            if !text.is_empty() {
                                copy_osc52(&text);
                            }
                            app.selection = None;
                        } else {
                            let now = Instant::now();
                            if app.last_ctrl_c.map_or(false, |t| now.duration_since(t) < Duration::from_millis(500)) {
                                app.should_quit = true;
                            } else {
                                app.last_ctrl_c = Some(now);
                            }
                        }
                    }
                    KeyCode::Esc => {
                        if app.autocomplete_visible {
                            app.autocomplete_visible = false;
                            app.autocomplete_items.clear();
                            app.autocomplete_selected = 0;
                        } else {
                            app.selection = None;
                        }
                    }
                    KeyCode::Enter => {
                        // Accept autocomplete selection if popup is open.
                        if app.autocomplete_visible
                            && app.autocomplete_selected < app.autocomplete_items.len()
                        {
                            let name = app.autocomplete_items[app.autocomplete_selected].name.clone();
                            app.input = format!("/{name} ");
                            app.cursor_pos = app.input.len();
                            app.autocomplete_visible = false;
                            app.autocomplete_items.clear();
                            app.autocomplete_selected = 0;
                        } else if !app.input.trim().is_empty() {
                            let prompt = app.input.clone();
                            app.input_history.push(prompt.clone());
                            const MAX_INPUT_HISTORY: usize = 500;
                            if app.input_history.len() > MAX_INPUT_HISTORY {
                                app.input_history.drain(0..app.input_history.len() - MAX_INPUT_HISTORY);
                            }
                            app.history_idx = None;
                            app.history_saved.clear();
                            app.chat_pinned = true;
                            app.input.clear();
                            app.cursor_pos = 0;
                            app.paste_markers.clear();
                            app.autocomplete_visible = false;
                            app.autocomplete_items.clear();
                            app.autocomplete_selected = 0;

                            // Dispatch slash commands; send everything else as a prompt.
                            if let Some((cmd_name, args)) = crate::commands::parse_slash_input(&prompt) {
                                match app.command_registry.resolve(&cmd_name) {
                                    Some(SlashCommand::Local(LocalCommand::Quit)) => {
                                        app.should_quit = true;
                                    }
                                    Some(SlashCommand::Local(LocalCommand::Clear)) => {
                                        app.chat.clear();
                                        app.tools.clear();
                                    }
                                    Some(SlashCommand::Local(LocalCommand::Help)) => {
                                        let text = app.command_registry.help_text();
                                        app.chat.push(ChatMessage::System(text));
                                    }
                                    Some(SlashCommand::Remote { name }) => {
                                        app.chat.push(ChatMessage::User(prompt.clone()));
                                        app.tools_pinned = true;
                                        app.tools_scroll = 0;
                                        app.run_phase = None;
                                        if app.ws_status != WsStatus::Connected {
                                            app.chat.push(ChatMessage::System("not connected to gateway".to_string()));
                                        } else if let Some(ref client) = ws_client {
                                            let client = client.clone();
                                            let session_id = session_id.to_string();
                                            let tx = tui_tx.clone();
                                            let args_value = if args.is_empty() {
                                                serde_json::Value::Null
                                            } else {
                                                serde_json::Value::String(args)
                                            };
                                            tokio::spawn(async move {
                                                let result = client.exec_command(&name, args_value, &session_id).await;
                                                let _ = tx.send(TuiEvent::CommandResult(result)).await;
                                            });
                                        }
                                    }
                                    None => {
                                        app.chat.push(ChatMessage::System(format!(
                                            "Unknown command: /{cmd_name} — type /help for available commands"
                                        )));
                                    }
                                }
                            } else {
                                // Regular prompt — send to agent.
                                app.chat.push(ChatMessage::User(prompt.clone()));
                                app.tools_pinned = true;
                                app.tools_scroll = 0;
                                app.run_phase = None;
                                if app.ws_status != WsStatus::Connected {
                                    app.chat.push(ChatMessage::System("not connected to gateway".to_string()));
                                } else if let Some(ref client) = ws_client {
                                    let client = client.clone();
                                    let session_id = session_id.to_string();
                                    let tx = tui_tx.clone();
                                    tokio::spawn(async move {
                                        let result = client.submit_prompt(&prompt, &session_id).await;
                                        let _ = tx.send(TuiEvent::SubmitResult(result)).await;
                                    });
                                } else {
                                    app.chat.push(ChatMessage::System("not connected to gateway".to_string()));
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        for m in &mut app.paste_markers {
                            if m.start >= app.cursor_pos {
                                m.start += c.len_utf8();
                                m.end += c.len_utf8();
                            }
                        }
                        app.input.insert(app.cursor_pos, c);
                        app.cursor_pos += c.len_utf8();
                        #[cfg(debug_assertions)]
                        assert_markers_valid(&app.input, &app.paste_markers);
                        update_autocomplete(app);
                    }
                    KeyCode::Tab => {
                        if app.autocomplete_visible && !app.autocomplete_items.is_empty() {
                            app.autocomplete_selected =
                                (app.autocomplete_selected + 1) % app.autocomplete_items.len();
                        }
                    }
                    KeyCode::BackTab => {
                        if app.autocomplete_visible && !app.autocomplete_items.is_empty() {
                            app.autocomplete_selected = app.autocomplete_selected
                                .checked_sub(1)
                                .unwrap_or(app.autocomplete_items.len() - 1);
                        }
                    }
                    KeyCode::Backspace => {
                        app.do_backspace();
                        update_autocomplete(app);
                    }
                    KeyCode::Delete => {
                        app.do_delete();
                        update_autocomplete(app);
                    }
                    KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let new_pos = word_left(&app.input, app.cursor_pos);
                        app.cursor_pos = app.paste_markers.iter()
                            .find(|m| new_pos > m.start && new_pos < m.end)
                            .map_or(new_pos, |m| m.start);
                    }
                    KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let new_pos = word_right(&app.input, app.cursor_pos);
                        app.cursor_pos = app.paste_markers.iter()
                            .find(|m| new_pos > m.start && new_pos < m.end)
                            .map_or(new_pos, |m| m.end);
                    }
                    KeyCode::Left => { app.move_left(); }
                    KeyCode::Right => { app.move_right(); }
                    KeyCode::Home => {
                        let disp = app.display_cursor();
                        let disp_start = visual_line_start(disp, app.input_inner_width);
                        app.cursor_pos = display_to_actual(disp_start, &app.paste_markers);
                    }
                    KeyCode::End => {
                        let disp = app.display_cursor();
                        let disp_len = app.display_input().len();
                        let disp_end = visual_line_end(disp, disp_len, app.input_inner_width);
                        app.cursor_pos = display_to_actual(disp_end, &app.paste_markers);
                    }
                    // shift+up/down/pgup/pgdown scroll chat; must appear before plain Up/Down
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        let current = if app.chat_pinned { app.chat_max_scroll } else { app.chat_scroll };
                        app.chat_pinned = false;
                        app.chat_scroll = current.saturating_sub(3);
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        let current = if app.chat_pinned { app.chat_max_scroll } else { app.chat_scroll };
                        let next = current.saturating_add(3);
                        if next >= app.chat_max_scroll {
                            app.chat_pinned = true;
                        } else {
                            app.chat_pinned = false;
                            app.chat_scroll = next;
                        }
                    }
                    KeyCode::PageUp => {
                        let current = if app.chat_pinned { app.chat_max_scroll } else { app.chat_scroll };
                        app.chat_pinned = false;
                        app.chat_scroll = current.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        let current = if app.chat_pinned { app.chat_max_scroll } else { app.chat_scroll };
                        let next = current.saturating_add(10);
                        if next >= app.chat_max_scroll {
                            app.chat_pinned = true;
                        } else {
                            app.chat_pinned = false;
                            app.chat_scroll = next;
                        }
                    }
                    KeyCode::Up => {
                        if !app.input_history.is_empty() {
                            let next_idx = match app.history_idx {
                                None => {
                                    app.history_saved = app.input.clone();
                                    app.input_history.len() - 1
                                }
                                Some(i) => i.saturating_sub(1),
                            };
                            app.history_idx = Some(next_idx);
                            app.input = app.input_history[next_idx].clone();
                            app.paste_markers.clear();
                            app.cursor_pos = app.input.len();
                        }
                    }
                    KeyCode::Down => {
                        match app.history_idx {
                            None => {}
                            Some(i) if i + 1 >= app.input_history.len() => {
                                app.history_idx = None;
                                app.input = app.history_saved.clone();
                                app.paste_markers.clear();
                                app.cursor_pos = app.input.len();
                            }
                            Some(i) => {
                                let next_idx = i + 1;
                                app.history_idx = Some(next_idx);
                                app.input = app.input_history[next_idx].clone();
                                app.paste_markers.clear();
                                app.cursor_pos = app.input.len();
                            }
                        }
                    }
                    _ => {}
                }
            }

            TuiEvent::Mouse(ev) => {
                match ev.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if in_rect(ev.column, ev.row, app.chat_area) {
                            match screen_to_chat_pos(ev.column, ev.row, app) {
                                Some(pos) => {
                                    app.selection = Some(Selection {
                                        target: SelectionTarget::Chat,
                                        anchor: pos,
                                        extent: pos,
                                    });
                                }
                                None => { app.selection = None; }
                            }
                        } else if app.tools_visible && in_rect(ev.column, ev.row, app.tools_area) {
                            match screen_to_tools_row(ev.row, app) {
                                Some(row_idx) => {
                                    app.selection = Some(Selection {
                                        target: SelectionTarget::Tools,
                                        anchor: ChatPos { line_idx: row_idx, col: 0 },
                                        extent: ChatPos { line_idx: row_idx, col: usize::MAX },
                                    });
                                }
                                None => { app.selection = None; }
                            }
                        } else {
                            app.selection = None;
                        }
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if in_rect(ev.column, ev.row, app.chat_area) {
                            let is_chat_sel = matches!(&app.selection, Some(s) if s.target == SelectionTarget::Chat);
                            if is_chat_sel {
                                let pos = screen_to_chat_pos(ev.column, ev.row, app);
                                if let (Some(pos), Some(ref mut sel)) = (pos, app.selection.as_mut()) {
                                    sel.extent = pos;
                                }
                            }
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        if let Some(ref sel) = app.selection {
                            if sel.anchor == sel.extent {
                                app.selection = None;
                            }
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        if in_rect(ev.column, ev.row, app.chat_area) {
                            let current = if app.chat_pinned { app.chat_max_scroll } else { app.chat_scroll };
                            app.chat_pinned = false;
                            app.chat_scroll = current.saturating_sub(3);
                        } else if app.tools_visible && in_rect(ev.column, ev.row, app.tools_area) {
                            let current = if app.tools_pinned { app.tools_max_scroll } else { app.tools_scroll };
                            app.tools_pinned = false;
                            app.tools_scroll = current.saturating_sub(1);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if in_rect(ev.column, ev.row, app.chat_area) {
                            let current = if app.chat_pinned { app.chat_max_scroll } else { app.chat_scroll };
                            let next = current.saturating_add(3);
                            if next >= app.chat_max_scroll {
                                app.chat_pinned = true;
                            } else {
                                app.chat_scroll = next;
                            }
                        } else if app.tools_visible && in_rect(ev.column, ev.row, app.tools_area) {
                            let current = if app.tools_pinned { app.tools_max_scroll } else { app.tools_scroll };
                            let next = current.saturating_add(1);
                            if next >= app.tools_max_scroll {
                                app.tools_pinned = true;
                            } else {
                                app.tools_scroll = next;
                            }
                        }
                    }
                    _ => {}
                }
            }

            TuiEvent::SubmitResult(result) => {
                match result {
                    Ok(run_id) => { app.current_run_id = Some(run_id); }
                    Err(e) => {
                        app.last_error = Some(e.clone());
                        app.chat.push(ChatMessage::System(format!("error: {e}")));
                    }
                }
            }

            TuiEvent::Agent(ev) => {
                app.handle_agent_event(ev);
            }
            TuiEvent::WsStatus(status) => {
                match &status {
                    WsStatus::Disconnected | WsStatus::Error(_) => {
                        if let WsStatus::Error(e) = &status {
                            app.last_error = Some(e.clone());
                            app.chat.push(ChatMessage::System(format!("WS error: {e}")));
                        } else {
                            app.chat.push(ChatMessage::System("disconnected from gateway".into()));
                        }
                        ws_client = None;
                        if !app.reconnecting {
                            app.reconnecting = true;
                            let gateway_url = app.gateway_url.clone();
                            let auth_token = app.auth_token.clone();
                            let tx = tui_tx.clone();
                            tokio::spawn(async move {
                                let mut backoff = Duration::from_millis(250);
                                loop {
                                    tokio::time::sleep(backoff).await;
                                    match WsClient::connect(&gateway_url, auth_token.clone(), tx.clone()).await {
                                        Ok(client) => {
                                            let _ = tx.send(TuiEvent::Reconnected(Ok(client))).await;
                                            return;
                                        }
                                        Err(e) => {
                                            let _ = tx.send(TuiEvent::Reconnected(Err(e))).await;
                                            backoff = (backoff * 2).min(Duration::from_secs(2));
                                        }
                                    }
                                }
                            });
                        }
                    }
                    _ => {}
                }
                app.ws_status = status;
            }

            TuiEvent::Reconnected(result) => {
                match result {
                    Ok(client) => {
                        // Re-fetch commands after reconnect.
                        let fetch_client = client.clone();
                        let tx = tui_tx.clone();
                        tokio::spawn(async move {
                            let result = fetch_client.list_commands().await;
                            let _ = tx.send(TuiEvent::CommandsLoaded(result)).await;
                        });
                        ws_client = Some(client);
                        app.reconnecting = false;
                        app.ws_status = WsStatus::Connected;
                        app.chat.push(ChatMessage::System(format!("reconnected to {}", app.gateway_url)));
                    }
                    Err(_) => {
                        // reconnect task is still running with backoff, nothing to do here
                    }
                }
            }
            TuiEvent::CommandsLoaded(result) => {
                if let Ok(payload) = result {
                    app.command_registry.set_remote(&payload);
                }
            }
            TuiEvent::CommandResult(result) => {
                match result {
                    Ok(payload) => {
                        let msg = format_command_result(&payload);
                        app.chat.push(ChatMessage::System(msg));
                    }
                    Err(e) => {
                        app.chat.push(ChatMessage::System(format!("command error: {e}")));
                    }
                }
                app.run_phase = None;
            }
            TuiEvent::Resize(_, _) => {}
        }

        app.compact_chat();

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
