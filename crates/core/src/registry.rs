//! Namespaced registry keys.
//!
//! Registry keys are stable string identifiers used for authoring and
//! data-driven logic (e.g., `mdm:stone`). They are ordered and validated to
//! support deterministic iteration and stable persistence.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Default namespace used when a key omits an explicit namespace.
pub const DEFAULT_NAMESPACE: &str = "mdm";

/// Error returned when parsing an invalid [`RegistryKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryKeyError {
    message: String,
}

impl RegistryKeyError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RegistryKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RegistryKeyError {}

/// A namespaced key of the form `namespace:path`.
///
/// Ordering is lexical by `(namespace, path)` and is stable across runs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RegistryKey {
    namespace: String,
    path: String,
}

impl RegistryKey {
    /// Parse a registry key.
    ///
    /// Accepts either:
    /// - `namespace:path`
    /// - `path` (uses [`DEFAULT_NAMESPACE`])
    pub fn parse(input: &str) -> Result<Self, RegistryKeyError> {
        Self::parse_with_default_namespace(input, DEFAULT_NAMESPACE)
    }

    /// Parse a registry key using a caller-provided default namespace.
    pub fn parse_with_default_namespace(
        input: &str,
        default_namespace: &str,
    ) -> Result<Self, RegistryKeyError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(RegistryKeyError::new("RegistryKey cannot be empty"));
        }

        let (namespace, path) = match input.split_once(':') {
            Some((ns, p)) => (ns, p),
            None => (default_namespace, input),
        };

        let namespace = namespace.trim();
        let path = path.trim();

        validate_namespace(namespace)?;
        validate_path(path)?;

        Ok(Self {
            namespace: namespace.to_string(),
            path: path.to_string(),
        })
    }

    /// Registry key namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Registry key path.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for RegistryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

impl FromStr for RegistryKey {
    type Err = RegistryKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

fn validate_namespace(ns: &str) -> Result<(), RegistryKeyError> {
    if ns.is_empty() {
        return Err(RegistryKeyError::new("RegistryKey namespace cannot be empty"));
    }
    if ns.len() > 64 {
        return Err(RegistryKeyError::new(
            "RegistryKey namespace too long (max 64)",
        ));
    }
    if !ns
        .chars()
        .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
    {
        return Err(RegistryKeyError::new(
            "RegistryKey namespace has invalid characters (allowed: a-z0-9_.-)",
        ));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), RegistryKeyError> {
    if path.is_empty() {
        return Err(RegistryKeyError::new("RegistryKey path cannot be empty"));
    }
    if path.len() > 128 {
        return Err(RegistryKeyError::new("RegistryKey path too long (max 128)"));
    }
    if !path.chars().all(|c| {
        matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/' )
    }) {
        return Err(RegistryKeyError::new(
            "RegistryKey path has invalid characters (allowed: a-z0-9_./-)",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaced_key() {
        let key = RegistryKey::parse("mdm:stone").unwrap();
        assert_eq!(key.namespace(), "mdm");
        assert_eq!(key.path(), "stone");
        assert_eq!(key.to_string(), "mdm:stone");
    }

    #[test]
    fn parses_with_default_namespace() {
        let key = RegistryKey::parse("stone").unwrap();
        assert_eq!(key.to_string(), "mdm:stone");
    }

    #[test]
    fn rejects_empty() {
        assert!(RegistryKey::parse("").is_err());
        assert!(RegistryKey::parse("   ").is_err());
    }

    #[test]
    fn rejects_invalid_chars() {
        assert!(RegistryKey::parse("mdm:Stone").is_err());
        assert!(RegistryKey::parse("MDM:stone").is_err());
        assert!(RegistryKey::parse("mdm:stone?").is_err());
        assert!(RegistryKey::parse("mdm:").is_err());
        assert!(RegistryKey::parse(":stone").is_err());
    }
}

