use std::io::{self, Write};

pub fn netstring(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 32);
    write_netstring(&mut out, bytes).expect("vec write cannot fail");
    out
}

pub struct NetstringWriter<W: Write> {
    inner: W,
}

impl<W: Write> NetstringWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn write_prefix(&mut self, prefix: &str) -> io::Result<()> {
        self.inner.write_all(prefix.as_bytes())
    }

    pub fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write_bytes(s.as_bytes())
    }

    pub fn write_int(&mut self, n: i64) -> io::Result<()> {
        self.write_bytes(n.to_string().as_bytes())
    }

    pub fn write_count(&mut self, n: usize) -> io::Result<()> {
        self.write_bytes(n.to_string().as_bytes())
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        write_netstring(&mut self.inner, bytes)
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

fn write_netstring<W: Write>(w: &mut W, bytes: &[u8]) -> io::Result<()> {
    write!(w, "{}:", bytes.len())?;
    w.write_all(bytes)?;
    w.write_all(b",")?;
    Ok(())
}

