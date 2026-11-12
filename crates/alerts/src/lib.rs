#![forbid(unsafe_code)]

//! `agentlens-alerts` crate.

#[derive(Debug, Default)]
pub struct AlertsModule;

impl AlertsModule {
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
        let _module = AlertsModule::new();
    }
}
