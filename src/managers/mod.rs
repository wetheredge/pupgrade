mod bun;
mod cargo;
mod github_actions;
mod utils;

use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;

use self::utils::Spanned;

pub fn all<S: Scanner>() -> Vec<Box<dyn Manager<S>>> {
    vec![
        Box::new(bun::Manager::new()),
        Box::new(cargo::Manager::new()),
        Box::new(github_actions::Manager::new()),
    ]
}

pub trait Manager<S: Scanner> {
    fn name(&self) -> &'static str;

    fn filter_directory(&self, path: &Path) -> bool {
        path.file_name().is_none_or(|name| !is_dotfile(name))
    }

    fn filter_file(&self, path: &Path) -> bool;

    fn scan_file(&mut self, file: &Path, scanner: S);
}

pub trait Scanner {
    fn register(&self, id: usize, name: &str, category: Cow<'static, str>, version: &str);
}

fn is_dotfile(file_name: &OsStr) -> bool {
    file_name
        .as_encoded_bytes()
        .starts_with(OsStr::new(".").as_encoded_bytes())
}
