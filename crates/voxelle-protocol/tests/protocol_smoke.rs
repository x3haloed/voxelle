use ed25519_dalek::{SigningKey, VerifyingKey};
use ed25519_dalek::pkcs8::EncodePublicKey;
use rand::rngs::OsRng;

#[test]
fn principal_id_is_stable_for_same_spki() {
    let sk = SigningKey::generate(&mut OsRng);
    let vk: VerifyingKey = sk.verifying_key();
    let spki = vk
        .to_public_key_der()
        .expect("spki")
        .as_bytes()
        .to_vec();

    let a = voxelle_protocol::principal_id_from_spki_der(&spki);
    let b = voxelle_protocol::principal_id_from_spki_der(&spki);
    assert_eq!(a, b);
    assert!(a.starts_with("ed25519:"));
}

#[test]
fn parse_ed25519_spki_roundtrip() {
    let sk = SigningKey::generate(&mut OsRng);
    let vk: VerifyingKey = sk.verifying_key();
    let spki = vk
        .to_public_key_der()
        .expect("spki")
        .as_bytes()
        .to_vec();

    assert!(voxelle_protocol::is_ed25519_spki(&spki));
    let parsed = voxelle_protocol::ed25519_public_key_from_spki_der(&spki).expect("parse");
    assert_eq!(parsed.as_bytes(), vk.as_bytes());
}

#[test]
fn netstring_writer_matches_expected_format() {
    let mut w = voxelle_protocol::NetstringWriter::new(Vec::<u8>::new());
    w.write_prefix("p2pspace/test/v0\n").unwrap();
    w.write_str("hi").unwrap();
    w.write_int(0).unwrap();
    w.write_bytes(b"").unwrap();
    let out = w.into_inner();
    assert_eq!(out, b"p2pspace/test/v0\n2:hi,1:0,0:,");
}
