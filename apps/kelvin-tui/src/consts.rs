use ratatui::style::Color;

// --- Network & Configuration ---
pub const DEFAULT_GATEWAY_URL: &str = "ws://127.0.0.1:34617";
pub const DEFAULT_SESSION_ID: &str = "main";
pub const ENV_VAR_GATEWAY_TOKEN: &str = "KELVIN_GATEWAY_TOKEN";

// --- Paste Handling ---
pub const PASTE_THRESHOLD_LINES: usize = 3;
pub const PASTE_THRESHOLD_BYTES: usize = 200;
pub const KB_DIVISOR: f64 = 1024.0;

// --- Chat Management ---
pub const MAX_CHAT_MESSAGES: usize = 1000;
pub const MAX_INPUT_HISTORY: usize = 500;

// --- Autocomplete ---
pub const MAX_VISIBLE: usize = 8;

// --- UI Colors & Styling ---
pub const HL_BG: Color = Color::Indexed(238);

// --- UI Layout ---
pub const INPUT_PREFIX_WIDTH: usize = 2; // "> "
pub const MIN_INNER_WIDTH: u16 = 3;
pub const MAX_INPUT_CONTENT_LINES: u16 = 5;
pub const INPUT_BORDER_WIDTH: u16 = 2;
pub const TOOLS_AREA_PERCENTAGE: u16 = 25;
pub const STATUS_BAR_HEIGHT: u16 = 1;
pub const TOOLS_HEADER_ROWS: u16 = 2; // top border + header row
pub const RGB_ORANGE: (u8, u8, u8) = (255, 165, 0);

// --- Text Processing ---
pub const BASE64_ALPHABET: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
pub const OSC52_PREFIX: &str = "\x1b]52;c;";
