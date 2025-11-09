use camino::Utf8Path;

use crate::dep_collector::{GroupFormat, Version};

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "galock"
    }

    fn walk_directory(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|dir| dir == ".github")
    }

    fn walk_file(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|name| name == "galock.toml")
    }

    fn scan_file(&self, _path: &Utf8Path, collector: crate::DepCollector<'_>) {
        let group = collector
            .get_or_push_group("GitHub Actions".into(), GroupFormat::Plain)
            .unwrap();

        let actions = duct::cmd!("galock", "list")
            .stdin_null()
            .stderr_null()
            .stdout_capture()
            .read()
            .unwrap();

        for line in actions.lines() {
            let (action, tag) = line.split_once('@').unwrap();
            group.push_dep(action.to_owned(), None, Version::SemVer(tag.to_owned()));
        }
    }
}
