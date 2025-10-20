use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::bytes::{Match, Regex};

pub(super) struct Manager {
    deps: Vec<Dep>,
    name_to_id: HashMap<String, usize>,
}

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "GitHub actions"
    }

    fn filter_directory(&self, path: &Path) -> bool {
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

    fn filter_file(&self, path: &Path) -> bool {
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

    fn scan_file(&mut self, file: &Path, deps: &crate::DepCollector) {
        let yaml = std::fs::read(file).unwrap();

        for captures in get_regex().captures_iter(&yaml) {
            let action = captures.name("repo").unwrap();
            let start = action.start();
            let action = match_to_string(action);

            let version = captures
                .name("tag")
                .unwrap_or_else(|| captures.name("rev").unwrap());
            let end = version.end();
            let version = match_to_string(version);

            let id = if let Some(id) = self.name_to_id.get(&*action) {
                *id
            } else {
                let id = self.deps.len();
                let action = action.into_owned();
                self.name_to_id.insert(action.clone(), id);
                self.deps.push(Dep {
                    action,
                    spans: Vec::new(),
                });
                id
            };

            let dep = &mut self.deps[id];
            dep.spans.push((file.to_owned(), start..end));

            deps.register(0, &dep.action, "actions".into(), &version);

            // TODO: runner images
        }
    }
}

impl Manager {
    pub(super) fn new() -> Self {
        Self {
            deps: Vec::new(),
            name_to_id: HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct Dep {
    action: String,
    spans: Vec<(PathBuf, Range<usize>)>,
}

fn get_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let re = r"(?m-u:^[^#\n]+[[:^word:]]uses:\s*(?<repo>[[[:word:]]-]+/[-[[:word:]]]+)@(?<rev>[[:xdigit:]]+)\s*(?:#\s*(?<tag>[-.[[:word:]]]+\s*)?)$)";
        Regex::new(re).unwrap()
    })
}

fn match_to_string<'a>(m: Match<'a>) -> Cow<'a, str> {
    String::from_utf8_lossy(m.as_bytes())
}
