mod bun;
mod cargo;
mod github_actions;

use camino::Utf8Path;

pub(crate) fn all() -> Vec<Box<dyn Manager>> {
    vec![
        Box::new(bun::Manager),
        Box::new(cargo::Manager),
        Box::new(github_actions::Manager),
    ]
}

pub(crate) trait Manager {
    fn name(&self) -> &'static str;

    fn walk_directory(&self, path: &Utf8Path) -> bool {
        path.file_name().is_none_or(|name| !name.starts_with('.'))
    }

    fn walk_file(&self, path: &Utf8Path) -> bool;

    fn scan_file(&self, path: &Utf8Path, collector: &crate::DepCollector);
}
