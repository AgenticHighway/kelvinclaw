use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ed25519_dalek::SigningKey;
use pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};

/// Generates a new ed25519 keypair in PKCS#8 PEM format.
/// Returns (private_pem, public_pem).
pub fn generate_ed25519_keypair() -> Result<(String, String)> {
    let raw_bytes: [u8; 32] = rand::random();
    let signing_key = SigningKey::from_bytes(&raw_bytes);

    let private_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .context("failed to encode private key as PKCS#8 PEM")?
        .to_string();

    let public_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .context("failed to encode public key as PEM")?;

    Ok((private_pem, public_pem))
}

/// Ensures memory keys exist at the standard paths, generating them if missing.
/// Returns (private_key_path, public_key_path).
pub fn ensure_memory_keys(home: &Path) -> Result<(PathBuf, PathBuf)> {
    let priv_path = home.join("memory-private.pem");
    let pub_path = home.join("memory-public.pem");

    if priv_path.exists() && pub_path.exists() {
        return Ok((priv_path, pub_path));
    }

    let (private_pem, public_pem) = generate_ed25519_keypair()?;

    // Write private key with restricted permissions on Unix.
    write_private_key(&priv_path, &private_pem)?;

    std::fs::write(&pub_path, public_pem.as_bytes())
        .with_context(|| format!("failed to write public key to {}", pub_path.display()))?;

    println!(
        "[kelvin] generated ed25519 memory keys: {} / {}",
        priv_path.display(),
        pub_path.display()
    );

    Ok((priv_path, pub_path))
}

#[cfg(unix)]
fn write_private_key(path: &Path, pem: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("failed to open {} for writing", path.display()))
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(pem.as_bytes())
                .with_context(|| format!("failed to write private key to {}", path.display()))
        })
}

#[cfg(not(unix))]
fn write_private_key(path: &Path, pem: &str) -> Result<()> {
    // On Windows, rely on the home-dir ACL for protection.
    std::fs::write(path, pem.as_bytes())
        .with_context(|| format!("failed to write private key to {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        sub: String,
        exp: u64,
    }

    /// Roundtrip test: generate a keypair, sign a JWT with the private key, verify with the public key.
    /// This validates compatibility with the memory controller's jsonwebtoken usage.
    #[test]
    fn test_ed25519_jwt_roundtrip() {
        let (private_pem, public_pem) = generate_ed25519_keypair().expect("keygen failed");

        let encoding_key =
            EncodingKey::from_ed_pem(private_pem.as_bytes()).expect("invalid private pem");
        let decoding_key =
            DecodingKey::from_ed_pem(public_pem.as_bytes()).expect("invalid public pem");

        let claims = Claims {
            sub: "test".to_string(),
            exp: 9999999999,
        };

        let token = jsonwebtoken::encode(&Header::new(Algorithm::EdDSA), &claims, &encoding_key)
            .expect("failed to sign JWT");

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = false;

        let decoded = jsonwebtoken::decode::<Claims>(&token, &decoding_key, &validation)
            .expect("failed to verify JWT");

        assert_eq!(decoded.claims.sub, "test");
    }
}
