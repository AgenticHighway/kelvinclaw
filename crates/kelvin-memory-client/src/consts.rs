// --- Network ---
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:50051";
pub const DEFAULT_HTTP_SCHEME: &str = "http";
pub const DEFAULT_HTTPS_SCHEME: &str = "https";
pub const HTTPS_PREFIX: &str = "https://";

// --- Authentication & Delegation ---
pub const DEFAULT_ISSUER: &str = "kelvin-root";
pub const DEFAULT_AUDIENCE: &str = "kelvin-memory-controller";
pub const DEFAULT_SUBJECT: &str = "kelvin-root-memory-client";
pub const SIGNING_ALGORITHM: &str = "Ed25519Sha512";

// --- Tenant Configuration ---
pub const DEFAULT_TENANT_ID: &str = "default";
pub const DEFAULT_WORKSPACE_ID: &str = "default";
pub const DEFAULT_SESSION_ID: &str = "default";
pub const DEFAULT_MODULE_ID: &str = "memory.echo";

// --- Timeouts & Limits ---
pub const DEFAULT_TIMEOUT_MS: u64 = 2_000;
pub const DEFAULT_MAX_BYTES: u64 = 1024 * 1024;
pub const DEFAULT_MAX_RESULTS: u32 = 20;
pub const TOKEN_EXPIRY_OFFSET_SECS: u64 = 60;

// --- Capabilities ---
pub const CAPABILITY_MEMORY_CRUD: &str = "memory_crud";
pub const CAPABILITY_MEMORY_READ: &str = "memory_read";
pub const CAPABILITY_MEMORY_HEALTH: &str = "memory_health";

// --- Memory Service Metadata ---
pub const BACKEND_TYPE: &str = "rpc";
pub const PROVIDER_NAME: &str = "kelvin-memory-controller";
pub const REQUESTED_PROVIDER_NAME: &str = "memory-controller";

// --- Environment Variable Names ---
pub const ENV_MEMORY_RPC_ENDPOINT: &str = "KELVIN_MEMORY_RPC_ENDPOINT";
pub const ENV_MEMORY_RPC_ISSUER: &str = "KELVIN_MEMORY_RPC_ISSUER";
pub const ENV_MEMORY_RPC_AUDIENCE: &str = "KELVIN_MEMORY_RPC_AUDIENCE";
pub const ENV_MEMORY_RPC_SUBJECT: &str = "KELVIN_MEMORY_RPC_SUBJECT";
pub const ENV_MEMORY_TENANT_ID: &str = "KELVIN_MEMORY_TENANT_ID";
pub const ENV_MEMORY_WORKSPACE_ID: &str = "KELVIN_MEMORY_WORKSPACE_ID";
pub const ENV_MEMORY_SESSION_ID: &str = "KELVIN_MEMORY_SESSION_ID";
pub const ENV_MEMORY_MODULE_ID: &str = "KELVIN_MEMORY_MODULE_ID";
pub const ENV_MEMORY_SIGNING_KEY_PEM: &str = "KELVIN_MEMORY_SIGNING_KEY_PEM";
pub const ENV_MEMORY_SIGNING_KEY_PATH: &str = "KELVIN_MEMORY_SIGNING_KEY_PATH";
pub const ENV_MEMORY_SIGNING_KMS_KEY_ID: &str = "KELVIN_MEMORY_SIGNING_KMS_KEY_ID";
pub const ENV_MEMORY_SIGNING_KMS_REGION: &str = "KELVIN_MEMORY_SIGNING_KMS_REGION";
pub const ENV_MEMORY_RPC_TLS_CA_PEM: &str = "KELVIN_MEMORY_RPC_TLS_CA_PEM";
pub const ENV_MEMORY_RPC_TLS_CA_PATH: &str = "KELVIN_MEMORY_RPC_TLS_CA_PATH";
pub const ENV_MEMORY_RPC_TLS_DOMAIN_NAME: &str = "KELVIN_MEMORY_RPC_TLS_DOMAIN_NAME";
pub const ENV_MEMORY_RPC_TLS_CLIENT_CERT_PEM: &str = "KELVIN_MEMORY_RPC_TLS_CLIENT_CERT_PEM";
pub const ENV_MEMORY_RPC_TLS_CLIENT_CERT_PATH: &str = "KELVIN_MEMORY_RPC_TLS_CLIENT_CERT_PATH";
pub const ENV_MEMORY_RPC_TLS_CLIENT_KEY_PEM: &str = "KELVIN_MEMORY_RPC_TLS_CLIENT_KEY_PEM";
pub const ENV_MEMORY_RPC_TLS_CLIENT_KEY_PATH: &str = "KELVIN_MEMORY_RPC_TLS_CLIENT_KEY_PATH";
pub const ENV_MEMORY_RPC_ALLOW_INSECURE_NON_LOOPBACK: &str =
    "KELVIN_MEMORY_RPC_ALLOW_INSECURE_NON_LOOPBACK";
pub const ENV_MEMORY_TIMEOUT_MS: &str = "KELVIN_MEMORY_TIMEOUT_MS";
pub const ENV_MEMORY_MAX_BYTES: &str = "KELVIN_MEMORY_MAX_BYTES";
pub const ENV_MEMORY_MAX_RESULTS: &str = "KELVIN_MEMORY_MAX_RESULTS";
pub const RPC_OPT_IN_ENV_VARS: &[&str] = &[
    ENV_MEMORY_RPC_ENDPOINT,
    ENV_MEMORY_SIGNING_KEY_PEM,
    ENV_MEMORY_SIGNING_KEY_PATH,
    ENV_MEMORY_SIGNING_KMS_KEY_ID,
    ENV_MEMORY_RPC_TLS_CA_PEM,
    ENV_MEMORY_RPC_TLS_CA_PATH,
    ENV_MEMORY_RPC_TLS_CLIENT_CERT_PEM,
    ENV_MEMORY_RPC_TLS_CLIENT_CERT_PATH,
    ENV_MEMORY_RPC_TLS_CLIENT_KEY_PEM,
    ENV_MEMORY_RPC_TLS_CLIENT_KEY_PATH,
];

// --- Boolean Parsing ---
pub const BOOL_TRUE_VALUES: &[&str] = &["1", "true", "yes", "on"];

// --- Host Loopback Identifiers ---
pub const LOOPBACK_NAME: &str = "localhost";
pub const LOOPBACK_IPV4: &str = "127.0.0.1";
pub const LOOPBACK_IPV6: &str = "::1";
