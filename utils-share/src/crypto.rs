use anyhow::Result;
use hex::ToHex;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use tracing::info;

const HASH_LEN: usize = 10;

pub fn generate_signature(
    body: &[u8],
    secret: &[u8],
    signature_type: &str,
) -> Result<String> {
    let signature = match signature_type {
        "sha1" => {
            let mut mac = Hmac::<Sha1>::new_from_slice(secret)
                .map_err(|e| anyhow::anyhow!("HMAC init error: {}", e))?;
            mac.update(body);
            let result = mac.finalize();
            hex::encode(result.into_bytes())
        }
        "sha256" => {
            let mut mac = Hmac::<Sha256>::new_from_slice(secret)
                .map_err(|e| anyhow::anyhow!("HMAC init error: {}", e))?;
            mac.update(body);
            let result = mac.finalize();
            hex::encode(result.into_bytes())
        }
        _ => return Err(anyhow::anyhow!("Invalid signature type")),
    };
    Ok(signature)
}

pub fn verify_signature(
    body: &[u8],
    signature: &str,
    secret: &[u8],
    signature_type: &str,
) -> Result<bool> {
    let expected_signature = generate_signature(body, secret, signature_type)?;

    info!("Expected signature: {:?}, given signature: {:?}", &expected_signature[..10], signature);

    Ok(expected_signature.eq(signature))
}

pub fn generate_short_hash(input: &str, secret_key: &str) -> String {
    generate_signature(input.as_bytes(), secret_key.as_bytes(), "sha256")
        .expect("Failed to generate signature")[..HASH_LEN].to_owned()
}

pub fn verify_short_hash(input: &str, hash: &str, secret_key: &str) -> bool {
    generate_short_hash(input, secret_key).eq(hash)
}

pub fn sha256_hash(data: &[u8]) -> String {
    Sha256::digest(data).encode_hex()
}

pub fn sha256_short_hash(data: &[u8]) -> String {
    let full_hash = sha256_hash(data);
    full_hash[..16].to_string()
} 