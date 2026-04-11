// --- API Versions ---
pub const MEMORY_API_VERSION: &str = "v1alpha1";

// --- JWT Configuration ---
pub const JWT_ALGORITHM: jsonwebtoken::Algorithm = jsonwebtoken::Algorithm::EdDSA;

// --- Operation Names ---
pub const OPERATION_UPSERT: &str = "upsert";
pub const OPERATION_QUERY: &str = "query";
pub const OPERATION_READ: &str = "read";
pub const OPERATION_DELETE: &str = "delete";
pub const OPERATION_HEALTH: &str = "health";
