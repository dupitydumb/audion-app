use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,      // user_id
    pub username: String,
    pub role: String,     // Admin, User, StreamOnly
    pub exp: usize,       // Expiration time (UNIX timestamp)
}

impl Claims {
    pub fn require_admin(&self) -> Result<(), (axum::http::StatusCode, &'static str)> {
        if self.role != "Admin" {
            return Err((axum::http::StatusCode::FORBIDDEN, "Administrator privileges required"));
        }
        Ok(())
    }

    pub fn require_non_stream_only(&self) -> Result<(), (axum::http::StatusCode, &'static str)> {
        if self.role == "StreamOnly" {
            return Err((axum::http::StatusCode::FORBIDDEN, "Action not allowed for stream-only accounts"));
        }
        Ok(())
    }
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub fn generate_token(user_id: &str, username: &str, role: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let exp_days = std::env::var("AUDION_JWT_EXPIRATION_DAYS")
        .ok()
        .and_then(|val| val.parse::<u64>().ok())
        .unwrap_or(7); // 7 days default
    
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + (exp_days * 24 * 3600);

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        exp: expiration as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let validation = Validation::default();
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

fn derive_key(secret: &str) -> [u8; 32] {
    let mut key = [0u8; 32];
    let bytes = secret.as_bytes();
    if bytes.is_empty() {
        return key;
    }
    for (i, &b) in bytes.iter().cycle().take(32).enumerate() {
        key[i] = b;
    }
    key
}

pub fn encrypt_subsonic_password(password: &str, secret: &str) -> Result<String, String> {
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
    use rand::Rng;

    let key_bytes = derive_key(secret);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;
    
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, password.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    
    Ok(hex::encode(combined))
}

pub fn decrypt_subsonic_password(encrypted_hex: &str, secret: &str) -> Result<String, String> {
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};

    let combined = hex::decode(encrypted_hex)
        .map_err(|e| format!("Invalid hex: {}", e))?;
    
    if combined.len() < 12 {
        return Err("Encrypted data too short".to_string());
    }
    
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let key_bytes = derive_key(secret);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;
    
    let nonce = Nonce::from_slice(nonce_bytes);
    let decrypted = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    
    String::from_utf8(decrypted)
        .map_err(|e| format!("Invalid UTF-8 in decrypted string: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsonic_password_encryption_decryption() {
        let secret = "my-test-secret-key-123456";
        let password = "superSecretPassword!";
        
        let encrypted = encrypt_subsonic_password(password, secret).unwrap();
        assert_ne!(password, encrypted);
        
        let decrypted = decrypt_subsonic_password(&encrypted, secret).unwrap();
        assert_eq!(password, decrypted);
    }
}

pub mod middleware;

