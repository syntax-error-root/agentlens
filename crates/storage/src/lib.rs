#![forbid(unsafe_code)]

//! `agentlens-storage` crate.

#[derive(Debug, Default)]
pub struct StorageModule;

impl StorageModule {
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
        let _module = StorageModule::new();
    }
}
