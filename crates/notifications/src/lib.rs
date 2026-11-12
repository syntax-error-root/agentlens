#![forbid(unsafe_code)]

//! `agentlens-notifications` crate.

#[derive(Debug, Default)]
pub struct NotificationsModule;

impl NotificationsModule {
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
        let _module = NotificationsModule::new();
    }
}
