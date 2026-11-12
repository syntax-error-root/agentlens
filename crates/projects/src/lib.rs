#![forbid(unsafe_code)]

//! `agentlens-projects` crate.

#[derive(Debug, Default)]
pub struct ProjectsModule;

impl ProjectsModule {
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
        let _module = ProjectsModule::new();
    }
}
