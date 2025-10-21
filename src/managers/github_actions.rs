use std::ffi::OsStr;

use camino::Utf8Path;

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "GitHub actions"
    }

    fn walk_directory(&self, path: &Utf8Path) -> bool {
        let mut components = path.iter();

        if !matches!(components.next(), Some(dir) if dir == OsStr::new(".github")) {
            return false;
        }

        match components.next() {
            // .github
            None => true,
            Some(dir) if dir == OsStr::new("actions") => match components.next() {
                // .github/actions
                None => true,
                // .github/actions/*
                Some(_) => components.next().is_none(),
            },
            // .github/workflows
            Some(dir) if dir == OsStr::new("workflows") => components.next().is_none(),
            _ => false,
        }
    }

    fn walk_file(&self, path: &Utf8Path) -> bool {
        if !path
            .extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            return false;
        }

        if path.starts_with(".github/actions") {
            return path.file_stem().is_some_and(|name| name == "action");
        }

        true
    }

    fn scan_file(&self, _path: &Utf8Path, _collector: &crate::DepCollector) {
        // TODO
    }
}
