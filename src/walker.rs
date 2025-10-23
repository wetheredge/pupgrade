use std::fs;
use std::io;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};

use crate::Manager;

pub(crate) fn walk(root: &Utf8Path, managers: &[Box<dyn Manager>]) -> Box<[Vec<Utf8PathBuf>]> {
    let raw = Walker::new(root, managers).walk();

    let mut sorted = std::iter::repeat_n(Vec::new(), managers.len()).collect::<Vec<_>>();

    for (id, path) in raw {
        sorted[usize::from(id)].push(path);
    }

    for paths in &mut sorted {
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
    root: &'a Utf8Path,
    managers: &'a [Box<dyn Manager>],

    ignore: gix_ignore::Search,
    out: Vec<(ManagerSet, Utf8PathBuf)>,
}

impl<'a> Walker<'a> {
    fn new(root: &'a Utf8Path, managers: &'a [Box<dyn Manager>]) -> Self {
        let mut ignore = gix_ignore::Search::from_git_dir(
            &root.join(".git").into_std_path_buf(),
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

    fn walk(mut self) -> Vec<(ManagerSet, Utf8PathBuf)> {
        // FIXME: handle overflow
        let all_managers = (1 << self.managers.len() as u8) - 1;
        self.step(self.root, all_managers);
        self.out
    }
}

impl Walker<'_> {
    fn relative<'a>(&self, path: &'a Utf8Path) -> Option<&'a Utf8Path> {
        path.strip_prefix(self.root).ok()
    }

    fn display_path(&self, path: &Utf8Path) -> impl std::fmt::Display {
        self.relative(path).map_or(path, |p| {
            if p == Utf8Path::new("") {
                Utf8Path::new(".")
            } else {
                p
            }
        })
    }

    fn add_ignore_file(&mut self, path: &Utf8Path) -> io::Result<()> {
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

    fn is_ignored(&self, path: &Utf8Path, is_dir: bool) -> bool {
        let case = gix_ignore::glob::pattern::Case::Sensitive;
        self.ignore
            .pattern_matching_relative_path(path.as_str().into(), Some(is_dir), case)
            .is_some()
    }

    fn step(&mut self, dir: &Utf8Path, enabled: ManagerSet) {
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

            let path = Utf8PathBuf::try_from(entry.path()).unwrap();
            let relative = self.relative(&path).unwrap();

            let Ok(file_type) = entry.file_type() else {
                log::warn!("failed to read type of file: {relative}");
                continue;
            };
            let file_type = FileType::from(file_type);

            if self.is_ignored(relative, file_type == FileType::Directory) {
                log::debug!("ignoring {relative}");
                continue;
            }

            match file_type {
                FileType::Unknown => {
                    log::warn!("could not determine type: {relative}");
                }
                FileType::Symlink => {
                    log::warn!("skipping symlink: {relative}");
                }
                FileType::Directory => {
                    let mut new_enabled = enabled;
                    for (id, manager) in self.managers.iter().enumerate() {
                        let mask = 1 << (id as ManagerSet);
                        if (enabled & mask) > 0 && !manager.filter_directory(relative) {
                            log::debug!("{}: disabling in {relative}", manager.name());
                            new_enabled ^= mask;
                        }
                    }

                    if new_enabled > 0 {
                        self.step(&path, new_enabled);
                    }
                }
                FileType::File => {
                    for (id, manager) in self.managers.iter().enumerate() {
                        let id = id as ManagerSet;
                        let mask = 1 << id;
                        if (enabled & mask) > 0 && manager.filter_file(relative) {
                            log::debug!("{}: registering {relative}", manager.name());
                            self.out.push((id, path.clone()));
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FileType {
    Directory,
    File,
    Symlink,
    Unknown,
}

impl From<fs::FileType> for FileType {
    fn from(from: fs::FileType) -> Self {
        if from.is_dir() {
            Self::Directory
        } else if from.is_file() {
            Self::File
        } else if from.is_symlink() {
            Self::Symlink
        } else {
            Self::Unknown
        }
    }
}
