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

        #[derive(facet::Facet)]
        struct Action<'a> {
            repo: &'a str,
            tag: &'a str,
            commit: &'a str,
        }

        let json = duct::cmd!("galock", "list", "--json")
            .stdin_null()
            .stderr_null()
            .stdout_capture()
            .read()
            .unwrap();
        let actions: Vec<Action> = facet_json::from_str(&json).unwrap();

        for action in actions {
            group.push_dep(
                action.repo.to_owned(),
                None,
                Version::GitPinnedTag {
                    repo: action.repo.to_owned(),
                    commit: action.commit.to_owned(),
                    tag: action.tag.to_owned(),
                },
            );
        }
    }
}
