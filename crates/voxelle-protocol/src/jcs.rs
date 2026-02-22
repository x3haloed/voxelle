use anyhow::{Context, Result};

pub fn jcs_bytes<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    let s = serde_jcs::to_string(value).context("serialize to JCS")?;
    Ok(s.into_bytes())
}

