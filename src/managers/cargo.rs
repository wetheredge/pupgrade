use std::path::Path;

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "Cargo"
    }

    fn filter_file(&self, path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "Cargo.toml")
    }
}
