#![forbid(unsafe_code)]

//! `agentlens-auth` crate.

#[derive(Debug, Default)]
pub struct AuthModule;

impl AuthModule {
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
        let _module = AuthModule::new();
    }
}
