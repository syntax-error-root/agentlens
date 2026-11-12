#![forbid(unsafe_code)]

//! `agentlens-observability` crate.

#[derive(Debug, Default)]
pub struct ObservabilityModule;

impl ObservabilityModule {
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
        let _module = ObservabilityModule::new();
    }
}
