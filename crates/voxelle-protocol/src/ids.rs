use base64::Engine;
use sha2::{Digest, Sha256};

pub fn principal_id_from_spki_der(spki_der: &[u8]) -> String {
    format!("ed25519:{}", base64url_sha256(spki_der))
}

pub fn space_id_from_spki_der(spki_der: &[u8]) -> String {
    format!("ed25519:{}", base64url_sha256(spki_der))
}

fn base64url_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}
