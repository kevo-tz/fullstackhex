//! Core business logic for FullStackHex.
//!
//! This crate is the domain layer — models, services, and use cases
//! that are independent of the web framework, database, or transport.

pub mod error;
pub mod time;

/// Placeholder: remove when real domain types are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    pub max_page_size: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { max_page_size: 100 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_reasonable() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.max_page_size, 100);
    }
}
