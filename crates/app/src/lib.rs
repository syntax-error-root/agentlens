#![forbid(unsafe_code)]

//! `agentlens-app` crate.

#[derive(Debug, Default)]
pub struct AppModule;

impl AppModule {
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
        let _module = AppModule::new();
    }
}
