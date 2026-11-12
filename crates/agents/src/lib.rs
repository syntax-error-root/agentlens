#![forbid(unsafe_code)]

//! `agentlens-agents` crate.

#[derive(Debug, Default)]
pub struct AgentsModule;

impl AgentsModule {
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
        let _module = AgentsModule::new();
    }
}
