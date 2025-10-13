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
        // FIXME: handle overflow
        let all_managers = (1 << self.managers.len() as u8) - 1;
        self.step(self.root, all_managers);
        self.out
    }
}

impl Walker<'_> {
    fn relative<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        path.strip_prefix(self.root).ok()
    }

    fn display_path(&self, path: &Path) -> impl std::fmt::Display {
        self.relative(path)
            .map(|p| {
                if p == Path::new("") {
                    Path::new(".")
                } else {
                    p
                }
            })
            .unwrap_or(path)
            .display()
    }

    fn add_ignore_file(&mut self, path: &Path) -> io::Result<()> {
        let contents = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };

        self.ignore.add_patterns_buffer(
            &contents,
            self.relative(path).unwrap(),
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
        log::trace!("entering {}", self.display_path(dir));

        let local_ignore = dir.join(".gitignore");
        if let Err(err) = self.add_ignore_file(&local_ignore) {
            log::warn!(
                "failed to read ignore file {}: {err}",
                self.display_path(&local_ignore)
            );
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => {
                log::warn!("failed to open dir {}: {err}", self.display_path(dir));
                return;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    log::warn!("failed to read entry of {}: {err}", self.display_path(dir));
                    continue;
                }
            };

            let path = entry.path();
            let relative = self.relative(&path).unwrap();

            let Ok(file_type) = entry.file_type() else {
                log::warn!("failed to read type of file: {}", relative.display());
                continue;
            };

            if self.is_ignored(relative, file_type.is_dir()) {
                log::debug!("ignoring {}", relative.display());
                continue;
            }

            if file_type.is_symlink() {
                log::warn!("skipping symlink: {}", relative.display());
            } else if file_type.is_dir() {
                let mut new_enabled = enabled;
                for (id, manager) in self.managers.iter().enumerate() {
                    let mask = 1 << (id as ManagerSet);
                    if (enabled & mask) > 0 && !manager.filter_directory(relative) {
                        log::debug!("{}: disabling in {}", manager.name(), relative.display());
                        new_enabled ^= mask;
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
                        log::debug!("{}: registering {}", manager.name(), relative.display());
                        self.out.push((id, path.clone()));
                    }
                }
            }
        }
    }
}
