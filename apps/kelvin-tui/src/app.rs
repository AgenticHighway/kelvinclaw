use std::time::{Duration, Instant};

use crossterm::{
    cursor::SetCursorStyle,
    event::{DisableBracketedPaste, EnableBracketedPaste, Event, EventStream, KeyCode, KeyEvent,
            KeyModifiers},
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::interval;

use crate::{ui, ws_client::WsClient, CliConfig};

// ── Data types ────────────────────────────────────────────────────────────────

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

// ── TUI event ─────────────────────────────────────────────────────────────────

pub enum TuiEvent {
    Key(KeyEvent),
    Paste(String),
    Agent(AgentEvent),
    WsStatus(WsStatus),
    #[allow(dead_code)]
    Resize(u16, u16), // 4A
    Tick,
}

// ── Chat message ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    Assistant { text: String, complete: bool },
    System(String),
}

// ── Tool execution entry ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolEntry {
    pub tool_name: String,
    pub phase: ToolPhase,
    pub summary: Option<String>,
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
}

// ── Paste marker ──────────────────────────────────────────────────────────────

/// Marks a byte range in `App::input` that is displayed as a compact label.
/// The actual text (for submission) lives in `input[start..end]`.
#[derive(Debug, Clone)]
pub struct PasteMarker {
    pub start: usize,  // byte start in input (inclusive)
    pub end: usize,    // byte end in input (exclusive)
    pub label: String, // e.g. "[pasted 7 lines · 412 B]"
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

// ── Display / cursor helpers ──────────────────────────────────────────────────

/// Build the display string: paste regions are replaced with their labels.
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

/// Map an actual byte position in `input` to a position in the display string.
pub fn actual_to_display(pos: usize, markers: &[PasteMarker]) -> usize {
    let mut disp = 0;
    let mut i = 0;
    for m in markers {
        if pos <= m.start {
            return disp + pos.saturating_sub(i);
        }
        disp += m.start.saturating_sub(i);
        if pos < m.end {
            // Inside the region: snap to end of label
            return disp + m.label.len();
        }
        disp += m.label.len();
        i = m.end;
    }
    disp + pos.saturating_sub(i)
}

/// Map a display position back to an actual byte position in `input`.
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
            // Inside label: snap to end of paste region
            return m.end;
        }
        disp += llen;
        i = m.end;
    }
    i + disp_pos.saturating_sub(disp)
}

// ── App state ─────────────────────────────────────────────────────────────────

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
    pub last_error: Option<String>,
    pub chat_scroll: usize,
    pub should_quit: bool,
    pub last_ctrl_c: Option<Instant>,
    // Input history
    pub input_history: Vec<String>,
    pub history_idx: Option<usize>,
    pub history_saved: String,
    // Visual line width of the input box inner area (updated each draw)
    pub input_inner_width: usize,
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
            last_error: None,
            chat_scroll: 0,
            should_quit: false,
            last_ctrl_c: None,
            input_history: Vec::new(),
            history_idx: None,
            history_saved: String::new(),
            input_inner_width: 0,
        }
    }

    /// The string that should be rendered in the input widget.
    pub fn display_input(&self) -> String {
        build_display(&self.input, &self.paste_markers)
    }

    /// Cursor position within the display string.
    pub fn display_cursor(&self) -> usize {
        actual_to_display(self.cursor_pos, &self.paste_markers)
    }

    /// Insert pasted text at the current cursor position.
    /// Creates a PasteMarker if the text exceeds the threshold.
    pub fn do_paste(&mut self, text: String) {
        // Remove any markers that overlap the insertion point
        self.paste_markers.retain(|m| m.end <= self.cursor_pos || m.start >= self.cursor_pos);

        let start = self.cursor_pos;
        let len = text.len();

        if is_large_paste(&text) {
            let label = paste_label(&text);
            self.input.insert_str(start, &text);
            // Shift existing markers that come after
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

    /// Backspace at the current cursor position, treating paste regions atomically.
    pub fn do_backspace(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        // Check if we're right at the end of a paste marker
        if let Some(idx) = self.paste_markers.iter().position(|m| m.end == self.cursor_pos) {
            let m = self.paste_markers.remove(idx);
            let removed = m.end - m.start;
            self.input.drain(m.start..m.end);
            self.cursor_pos = m.start;
            // Shift subsequent markers
            for later in &mut self.paste_markers {
                if later.start >= m.end {
                    later.start -= removed;
                    later.end -= removed;
                }
            }
        } else {
            // 3C: find previous char boundary
            let mut prev = self.cursor_pos - 1;
            while prev > 0 && !self.input.is_char_boundary(prev) {
                prev -= 1;
            }
            let removed_len = self.cursor_pos - prev;
            self.input.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
            // Shift markers after deletion point
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

    /// Move cursor left, jumping over paste regions atomically.
    pub fn move_left(&mut self) {
        if self.cursor_pos == 0 { return; }
        if let Some(m) = self.paste_markers.iter().find(|m| m.end == self.cursor_pos) {
            self.cursor_pos = m.start;
        } else {
            // 3A: find previous char boundary
            let mut new_pos = self.cursor_pos - 1;
            while new_pos > 0 && !self.input.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.cursor_pos = new_pos;
        }
    }

    /// Move cursor right, jumping over paste regions atomically.
    pub fn move_right(&mut self) {
        if self.cursor_pos >= self.input.len() { return; }
        if let Some(m) = self.paste_markers.iter().find(|m| m.start == self.cursor_pos) {
            self.cursor_pos = m.end;
        } else {
            // 3B: find next char boundary
            let mut new_pos = self.cursor_pos + 1;
            while new_pos < self.input.len() && !self.input.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            self.cursor_pos = new_pos;
        }
    }

    /// Compact chat to at most MAX_CHAT_MESSAGES, prepending an omission notice.
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
                self.chat_scroll = usize::MAX;
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

// ── Input movement helpers ────────────────────────────────────────────────────

// 3D: char-based word detection (handles non-ASCII)
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Jump left to the start of the previous word (stops at any non-word boundary).
fn word_left(s: &str, pos: usize) -> usize {
    let mut current = pos;
    // Skip non-word chars
    for c in s[..current].chars().rev() {
        if is_word_char(c) { break; }
        current -= c.len_utf8();
    }
    // Skip word chars
    for c in s[..current].chars().rev() {
        if !is_word_char(c) { break; }
        current -= c.len_utf8();
    }
    current
}

/// Jump right to the end of the current/next word.
fn word_right(s: &str, pos: usize) -> usize {
    let mut current = pos;
    // Skip non-word chars
    for c in s[current..].chars() {
        if is_word_char(c) { break; }
        current += c.len_utf8();
    }
    // Skip word chars
    for c in s[current..].chars() {
        if !is_word_char(c) { break; }
        current += c.len_utf8();
    }
    current
}

// 5B: debug assertion to verify paste markers are valid
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

/// Start of the visual line containing `display_pos` in the display string.
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

/// End of the visual line containing `display_pos` (clamped to `display_len`).
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

// ── Terminal cleanup guard ────────────────────────────────────────────────────

/// Restores the terminal to its original state.  Implemented as a `Drop` guard
/// so cleanup runs even if the code panics or returns early via `?`.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Each step is independent — a failure in one must not skip the rest.
        if let Err(e) = crossterm::terminal::disable_raw_mode() {
            eprintln!("warn: disable_raw_mode failed: {e}");
        }
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableBracketedPaste,
            crossterm::cursor::Show,
        ) {
            eprintln!("warn: terminal restore failed: {e}");
        }
    }
}

// ── Main run loop ─────────────────────────────────────────────────────────────

pub async fn run(config: CliConfig) -> Result<(), String> {
    let (tui_tx, mut tui_rx) = mpsc::channel::<TuiEvent>(256);

    enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
    let mut stdout = std::io::stdout();
    // 6A: set cursor style once at startup (not per-frame)
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, SetCursorStyle::SteadyBlock)
        .map_err(|e| format!("enter alt screen: {e}"))?;

    // Guard is created immediately after raw mode is enabled.
    // It will restore the terminal when it is dropped for any reason.
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| format!("terminal: {e}"))?;

    let mut app = App::new(config.gateway_url.clone());

    let ws_result = WsClient::connect(&config.gateway_url, config.auth_token.clone(), tui_tx.clone()).await;
    let ws_client = match ws_result {
        Ok(client) => {
            app.ws_status = WsStatus::Connected;
            app.chat.clear();
            app.chat.push(ChatMessage::System(format!("Connected to {}", config.gateway_url)));
            Some(client)
        }
        Err(e) => {
            app.ws_status = WsStatus::Error(e.clone());
            app.last_error = Some(e.clone());
            app.chat.push(ChatMessage::System(format!("Connection failed: {e}")));
            None
        }
    };

    // Keyboard/paste event task — keep the handle so we can abort it before cleanup.
    let tui_tx_key = tui_tx.clone();
    let key_task = tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(Ok(event)) = reader.next().await {
            match event {
                Event::Key(key) => { let _ = tui_tx_key.send(TuiEvent::Key(key)).await; }
                Event::Paste(text) => { let _ = tui_tx_key.send(TuiEvent::Paste(text)).await; }
                Event::Resize(w, h) => { let _ = tui_tx_key.send(TuiEvent::Resize(w, h)).await; } // 4A
                _ => {}
            }
        }
    });

    // Tick timer
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

    let result = run_loop(&mut terminal, &mut app, &mut tui_rx, ws_client, &config.session_id).await;

    // Abort background tasks before restoring the terminal.  The event-reader
    // task owns crossterm's EventStream; aborting it first prevents any race
    // where the stream touches terminal state concurrently with our cleanup.
    key_task.abort();
    tick_task.abort();
    // Drop the terminal explicitly so its backend (stdout) is released before
    // the TerminalGuard runs its Drop impl.
    drop(terminal);

    // _guard is dropped here, running TerminalGuard::drop() which restores
    // raw mode, exits alt screen, disables bracketed paste, and shows cursor.
    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    tui_rx: &mut mpsc::Receiver<TuiEvent>,
    ws_client: Option<WsClient>,
    session_id: &str,
) -> Result<(), String> {
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
                    // ── Exit ────────────────────────────────────────────────
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let now = Instant::now();
                        if app.last_ctrl_c.map_or(false, |t| now.duration_since(t) < Duration::from_millis(500)) {
                            app.should_quit = true;
                        } else {
                            app.last_ctrl_c = Some(now);
                        }
                    }
                    // ── Submit ───────────────────────────────────────────────
                    KeyCode::Enter => {
                        if !app.input.trim().is_empty() { // 5A: reject whitespace-only
                            let prompt = app.input.clone();
                            app.input_history.push(prompt.clone());
                            // 2B: cap input history at 500 entries
                            const MAX_INPUT_HISTORY: usize = 500;
                            if app.input_history.len() > MAX_INPUT_HISTORY {
                                app.input_history.drain(0..app.input_history.len() - MAX_INPUT_HISTORY);
                            }
                            app.history_idx = None;
                            app.history_saved.clear();
                            app.chat.push(ChatMessage::User(prompt.clone()));
                            app.chat_scroll = usize::MAX;
                            app.input.clear();
                            app.cursor_pos = 0;
                            app.paste_markers.clear();
                            app.tools.clear();
                            app.run_phase = None;

                            if let Some(ref client) = ws_client {
                                match client.submit_prompt(&prompt, session_id).await {
                                    Ok(run_id) => { app.current_run_id = Some(run_id); }
                                    Err(e) => {
                                        app.last_error = Some(e.clone());
                                        app.chat.push(ChatMessage::System(format!("Error: {e}")));
                                    }
                                }
                            } else {
                                app.chat.push(ChatMessage::System("Not connected to gateway".to_string()));
                            }
                        }
                    }
                    // ── Character insert ─────────────────────────────────────
                    KeyCode::Char(c) => {
                        // Shift any markers after insertion point
                        for m in &mut app.paste_markers {
                            if m.start >= app.cursor_pos {
                                m.start += c.len_utf8();
                                m.end += c.len_utf8();
                            }
                        }
                        app.input.insert(app.cursor_pos, c);
                        app.cursor_pos += c.len_utf8();
                        #[cfg(debug_assertions)]
                        assert_markers_valid(&app.input, &app.paste_markers); // 5B
                    }
                    // ── Backspace ─────────────────────────────────────────────
                    KeyCode::Backspace => {
                        app.do_backspace();
                    }
                    // ── Horizontal movement ──────────────────────────────────
                    KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Word left on actual input, but snap out of any paste region
                        let new_pos = word_left(&app.input, app.cursor_pos);
                        // If we landed inside a marker, jump to its start
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
                    // ── Chat scroll ──────────────────────────────────────────
                    // 4B: modifier-based scroll keys (must be before plain Up/Down)
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.chat_scroll = app.chat_scroll.saturating_sub(3);
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.chat_scroll = app.chat_scroll.saturating_add(3);
                    }
                    KeyCode::PageUp => {
                        app.chat_scroll = app.chat_scroll.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.chat_scroll = app.chat_scroll.saturating_add(10);
                    }
                    // ── History navigation ───────────────────────────────────
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

            TuiEvent::Agent(ev) => {
                app.handle_agent_event(ev);
            }
            TuiEvent::WsStatus(status) => {
                // Group 7: consistent error routing
                match &status {
                    WsStatus::Disconnected => {
                        app.chat.push(ChatMessage::System("Disconnected from gateway".into()));
                    }
                    WsStatus::Error(e) => {
                        app.last_error = Some(e.clone());
                        app.chat.push(ChatMessage::System(format!("WS error: {e}")));
                    }
                    _ => {}
                }
                app.ws_status = status;
            }
            TuiEvent::Resize(_, _) => {} // 4A: redraw happens on next loop iteration
        }

        // 2A: compact chat to bounded size after every event
        app.compact_chat();

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
