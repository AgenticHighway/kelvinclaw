// --- Environment Variables ---
pub const ENV_BIND_ADDR: &str = "KELVIN_PLUGIN_REGISTRY_BIND";
pub const ENV_INDEX_PATH: &str = "KELVIN_PLUGIN_REGISTRY_INDEX";
pub const ENV_TRUST_POLICY_PATH: &str = "KELVIN_PLUGIN_REGISTRY_TRUST_POLICY";

// --- Network ---
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:34619";

// --- CLI Flags ---
pub const FLAG_BIND: &str = "--bind";
pub const FLAG_INDEX: &str = "--index";
pub const FLAG_TRUST_POLICY: &str = "--trust-policy";
pub const FLAG_HELP_SHORT: &str = "-h";
pub const FLAG_HELP_LONG: &str = "--help";

// --- API Routes ---
pub const ROUTE_HEALTH: &str = "/healthz";
pub const ROUTE_INDEX: &str = "/v1/index.json";
pub const ROUTE_PLUGINS: &str = "/v1/plugins";
pub const ROUTE_PLUGIN_VERSIONS: &str = "/v1/plugins/{plugin_id}";
pub const ROUTE_TRUST_POLICY: &str = "/v1/trust-policy";

// --- Schema ---
pub const SCHEMA_VERSION: &str = "v1";
