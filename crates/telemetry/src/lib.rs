#![forbid(unsafe_code)]

//! `agentlens-telemetry` crate.

#[derive(Debug, Default)]
pub struct TelemetryModule;

impl TelemetryModule {
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
        let _module = TelemetryModule::new();
    }
}
