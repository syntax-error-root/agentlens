#![forbid(unsafe_code)]

//! `agentlens-privacy` crate.

#[derive(Debug, Default)]
pub struct PrivacyModule;

impl PrivacyModule {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_init() {
        let _module = PrivacyModule::new();
    }
}
