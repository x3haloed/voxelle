use anyhow::{anyhow, Context, Result};
use ed25519_dalek::VerifyingKey;
use spki::der::Decode;
use spki::SubjectPublicKeyInfoRef;

const OID_ED25519: spki::ObjectIdentifier = spki::ObjectIdentifier::new_unwrap("1.3.101.112");

pub fn is_ed25519_spki(spki_der: &[u8]) -> bool {
    let Ok(spki) = SubjectPublicKeyInfoRef::from_der(spki_der) else {
        return false;
    };
    spki.algorithm.oid == OID_ED25519
}

pub fn ed25519_public_key_from_spki_der(spki_der: &[u8]) -> Result<VerifyingKey> {
    let spki =
        SubjectPublicKeyInfoRef::from_der(spki_der).context("parse SPKI SubjectPublicKeyInfo")?;

    if spki.algorithm.oid != OID_ED25519 {
        return Err(anyhow!("SPKI algorithm OID is not Ed25519"));
    }

    let pk_bytes = spki
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| anyhow!("SPKI subject_public_key missing"))?;

    let pk: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| anyhow!("Ed25519 public key must be 32 bytes (got {})", pk_bytes.len()))?;
    Ok(VerifyingKey::from_bytes(&pk)?)
}
