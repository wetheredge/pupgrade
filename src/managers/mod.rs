mod bun;
mod cargo;
mod github_actions;

use std::path::Path;

pub fn all() -> Vec<Box<dyn Manager>> {
    vec![
        Box::new(bun::Manager),
        Box::new(cargo::Manager),
        Box::new(github_actions::Manager),
    ]
}

pub trait Manager {
    fn name(&self) -> &'static str;

    fn filter_directory(&self, _path: &Path) -> bool {
        true
    }

    fn filter_file(&self, path: &Path) -> bool;
}
