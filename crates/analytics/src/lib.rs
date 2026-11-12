#![forbid(unsafe_code)]

//! `agentlens-analytics` crate.

#[derive(Debug, Default)]
pub struct AnalyticsModule;

impl AnalyticsModule {
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
        let _module = AnalyticsModule::new();
    }
}
