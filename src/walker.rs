use std::io;
use std::ops::Deref as _;
use std::path::{Path, PathBuf};

use crate::Manager;

pub fn walk<'m>(
    root: &Path,
    managers: &'m [Box<dyn Manager>],
) -> Box<[(&'m dyn Manager, Vec<PathBuf>)]> {
    let raw = Walker::new(root, managers).walk();

    let mut sorted = managers
        .iter()
        .map(|manager| (manager.deref(), Vec::new()))
        .collect::<Vec<_>>();

    for (id, path) in raw {
        sorted[usize::from(id)].1.push(path);
    }

    for (_, paths) in &mut sorted {
        paths.sort();
    }

    sorted.into_boxed_slice()
}

type ManagerSet = u8;

const IGNORE_SETTINGS: gix_ignore::search::Ignore = gix_ignore::search::Ignore {
    support_precious: false,
};
static IMPLICIT_IGNORES: &[u8] = br"
.git/
.gitignore
";

struct Walker<'a> {
    root: &'a Path,
    managers: &'a [Box<dyn Manager>],

    ignore: gix_ignore::Search,
    out: Vec<(ManagerSet, PathBuf)>,
}

impl<'a> Walker<'a> {
    fn new(root: &'a Path, managers: &'a [Box<dyn Manager>]) -> Self {
        let mut ignore = gix_ignore::Search::from_git_dir(
            &root.join(".git"),
            None,
            &mut Vec::new(),
            IGNORE_SETTINGS,
        )
        .unwrap();
        ignore.add_patterns_buffer(
            IMPLICIT_IGNORES,
            "<implicit ignores>",
            None,
            IGNORE_SETTINGS,
        );

        // TODO: add global git ignore

        Self {
            root,
            managers,
            ignore,
            out: Vec::new(),
        }
    }

    fn walk(mut self) -> Vec<(ManagerSet, PathBuf)> {
        self.step(self.root, ManagerSet::MAX);
        self.out
    }
}

impl Walker<'_> {
    fn relative<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        path.strip_prefix(self.root).ok()
    }

    fn add_ignore_file(&mut self, path: PathBuf) -> io::Result<()> {
        let contents = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };

        self.ignore.add_patterns_buffer(
            &contents,
            self.relative(&path).unwrap(),
            Some(Path::new("")),
            IGNORE_SETTINGS,
        );

        Ok(())
    }

    fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        // TODO: use encoded bytes?
        let path = &*path.to_string_lossy();
        let case = gix_ignore::glob::pattern::Case::Sensitive;
        self.ignore
            .pattern_matching_relative_path(path.into(), Some(is_dir), case)
            .is_some()
    }

    fn step(&mut self, dir: &Path, enabled: ManagerSet) {
        // TODO: logging
        let _ = self.add_ignore_file(dir.join(".gitignore"));

        let Ok(entries) = std::fs::read_dir(dir) else {
            // TODO: logging
            return;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                // TODO: logging
                continue;
            };

            let path = entry.path();
            let relative = self.relative(&path).unwrap();
            let Ok(file_type) = entry.file_type() else {
                // TODO: logging
                continue;
            };

            if self.is_ignored(relative, file_type.is_dir()) {
                continue;
            }

            if file_type.is_symlink() {
                eprintln!("Skipping symlink: {}", relative.display());
            } else if file_type.is_dir() {
                let mut new_enabled = 0;
                for (id, manager) in self.managers.iter().enumerate() {
                    let mask = 1 << (id as ManagerSet);
                    if (enabled & mask) > 0 && manager.filter_directory(relative) {
                        new_enabled |= mask;
                    }
                }

                if new_enabled > 0 {
                    self.step(&path, new_enabled);
                }
            } else {
                for (id, manager) in self.managers.iter().enumerate() {
                    let id = id as ManagerSet;
                    let mask = 1 << id;
                    if (enabled & mask) > 0 && manager.filter_file(relative) {
                        self.out.push((id, path.clone()));
                    }
                }
            }
        }
    }
}
