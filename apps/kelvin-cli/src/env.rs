/// Loads dotenv files in priority order (#102 — ~/.kelvinclaw/.env is canonical).
///
/// Search order (first match wins per key):
///   1. ~/.kelvinclaw/.env.local
///   2. ~/.kelvinclaw/.env
///   3. ./.env.local
///   4. ./.env
///
/// Variables already in the environment are never overwritten.
pub fn load_dotenv() {
    let home = crate::paths::kelvin_home();

    let candidates = [
        home.join(".env.local"),
        home.join(".env"),
        std::path::PathBuf::from(".env.local"),
        std::path::PathBuf::from(".env"),
    ];

    for path in &candidates {
        if !path.exists() {
            continue;
        }
        match dotenvy::from_path_iter(path) {
            Ok(iter) => {
                for item in iter {
                    match item {
                        Ok((key, value)) => {
                            // Only set if not already present in environment.
                            if std::env::var(&key).is_err() {
                                // SAFETY: single-threaded at this point in startup.
                                unsafe { std::env::set_var(&key, &value) };
                            }
                        }
                        Err(_) => {} // Skip malformed lines silently
                    }
                }
            }
            Err(_) => {} // Skip unreadable files silently
        }
    }
}
