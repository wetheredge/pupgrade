mod basic_dep;
mod bun;
mod cargo;
mod github_actions;
mod utils;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use self::basic_dep::BasicDep;
use self::utils::Spanned;

pub(crate) fn all() -> Vec<Box<dyn Manager>> {
    vec![
        Box::new(bun::Manager::new()),
        Box::new(cargo::Manager::new()),
        Box::new(github_actions::Manager::new()),
    ]
}

pub(crate) trait Manager {
    fn name(&self) -> &'static str;

    fn filter_directory(&self, path: &Path) -> bool {
        path.file_name().is_none_or(|name| !is_dotfile(name))
    }

    fn filter_file(&self, path: &Path) -> bool;

    fn scan_file(&mut self, file: &Path);

    fn summary(&self, context: &SummaryContext) -> crate::summary::Node;
}

pub(crate) struct SummaryContext {
    pub(crate) root: PathBuf,
}

fn is_dotfile(file_name: &OsStr) -> bool {
    file_name
        .as_encoded_bytes()
        .starts_with(OsStr::new(".").as_encoded_bytes())
}
