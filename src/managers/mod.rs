mod basic_dep;
mod bun;
mod cargo;
mod github_actions;
mod utils;

use camino::{Utf8Path, Utf8PathBuf};

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

    fn filter_directory(&self, path: &Utf8Path) -> bool {
        path.file_name().is_none_or(|name| !name.starts_with('.'))
    }

    fn filter_file(&self, path: &Utf8Path) -> bool;

    fn scan_file(&mut self, file: &Utf8Path);

    fn summary(&self, context: &SummaryContext) -> crate::summary::Node;
}

pub(crate) struct SummaryContext {
    pub(crate) root: Utf8PathBuf,
}
