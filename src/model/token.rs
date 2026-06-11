use std::fmt;

#[derive(Clone, Eq, PartialEq)]
pub struct RuntimeTokenHash(String);

impl RuntimeTokenHash {
    pub fn from_token(token: &str) -> Self {
        Self(blake3::hash(token.as_bytes()).to_hex().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RuntimeTokenHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RuntimeTokenHash([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_tokens_without_exposing_them_in_debug_output() {
        let hash = RuntimeTokenHash::from_token("private-token");
        assert_ne!(hash.as_str(), "private-token");
        assert_eq!(format!("{hash:?}"), "RuntimeTokenHash([REDACTED])");
    }
}
