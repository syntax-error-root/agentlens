#![forbid(unsafe_code)]

//! `agentlens-domain` crate.

#[derive(Debug, Default)]
pub struct DomainModule;

impl DomainModule {
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
        let _module = DomainModule::new();
    }
}
