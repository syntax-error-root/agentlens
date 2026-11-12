#![forbid(unsafe_code)]

//! `agentlens-traces` crate.

#[derive(Debug, Default)]
pub struct TracesModule;

impl TracesModule {
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
        let _module = TracesModule::new();
    }
}
