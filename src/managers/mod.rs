mod bun;
mod cargo;
mod github_actions;

use std::ffi::OsStr;
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

    fn filter_directory(&self, path: &Path) -> bool {
        path.file_name().is_none_or(|name| !is_dotfile(name))
    }

    fn filter_file(&self, path: &Path) -> bool;
}

fn is_dotfile(file_name: &OsStr) -> bool {
    file_name
        .as_encoded_bytes()
        .starts_with(OsStr::new(".").as_encoded_bytes())
}
