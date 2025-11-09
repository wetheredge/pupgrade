use std::fmt;

use camino::Utf8Path;
use taplo::dom::{Node, node};

use crate::dep_collector::{GroupFormat, GroupHandle};

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "Cargo"
    }

    fn walk_file(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|name| name == "Cargo.toml")
    }

    fn scan_file(&self, path: &Utf8Path, collector: crate::DepCollector<'_>) {
        let root = collector
            .get_or_push_group("Cargo".into(), GroupFormat::Plain)
            .unwrap();
        let path_string = path.as_str().to_owned();
        let group = root
            .new_subgroup(path_string.clone(), GroupFormat::Path)
            .unwrap();

        let toml = std::fs::read_to_string(path).unwrap();

        let dom = taplo::parser::parse(&toml).into_dom();
        let root = dom.as_table().unwrap();

        let get_root_table = |key| get_table(root, &[], key, path);

        if let Some(workspace) = get_root_table("workspace")
            && let Some(dependencies) = get_table(&workspace, &["workspace"], "dependencies", path)
        {
            let group = group
                .new_subgroup("workspace".to_owned(), GroupFormat::Code)
                .unwrap();
            scan_inner(&group, &dependencies);
        }

        for key in ["dependencies", "build-dependencies", "dev-dependencies"] {
            if let Some(table) = get_root_table(key) {
                let group = group
                    .new_subgroup(key.to_owned(), GroupFormat::Code)
                    .unwrap();
                scan_inner(&group, &table);
            }
        }

        let each_nested_table = |root, run: &mut dyn FnMut(&str, &node::Table)| {
            if let Some(parent) = get_root_table(root) {
                for (child, table) in parent.entries().read().iter() {
                    let child = child.value();
                    if let Some(table) = table.as_table() {
                        run(child, table);
                    } else {
                        log::warn!("{path}: {root}.'{child}' is not a table");
                    }
                }
            }
        };

        each_nested_table("target", &mut |target, table| {
            for key in ["dependencies", "build-dependencies"] {
                if let Some(dependencies) = get_table(table, &["target", target], key, path) {
                    let group = group.get_group(key).unwrap().unwrap();
                    let group = group
                        .new_subgroup(target.to_owned(), GroupFormat::Code)
                        .unwrap();
                    scan_inner(&group, &dependencies);
                }
            }
        });

        each_nested_table("patch", &mut |registry, table| {
            // TODO: patch group?
            let group = group
                .new_subgroup(registry.to_owned(), GroupFormat::Code)
                .unwrap();
            scan_inner(&group, table);
        });
    }
}

fn scan_inner(group: &GroupHandle, table: &node::Table) {
    use crate::dep_collector::Version;

    for (name, meta) in table.entries().read().iter() {
        let version = match meta {
            Node::Table(meta) => {
                if let Some(version) = meta.get("version") {
                    let version = version.as_str().unwrap();
                    Version::SemVer(version.value().to_owned())
                } else if let Some(_repo) = meta.get("git") {
                    continue;
                    // TODO:
                    // let repo = spanned_str_node(repo.as_str().unwrap());
                    // let revision = meta.get("rev").as_ref().map(spanned_node_as_str);
                    // let tag = meta.get("tag").as_ref().map(spanned_node_as_str);
                    // Version::Git {
                    //     repo,
                    //     revision,
                    //     tag,
                    // }
                } else {
                    if let Some(workspace) = meta.get("workspace") {
                        let workspace = workspace.as_bool().unwrap();
                        if workspace.value() {
                            continue;
                        }
                    }

                    todo!()
                }
            }
            Node::Str(version) => Version::SemVer(version.value().to_owned()),

            _ => todo!(),
        };

        group.push_dep(name.value().to_owned(), None, version);
    }
}

struct FullKey<'a> {
    parents: &'a [&'a str],
    key: &'a str,
}

impl fmt::Display for FullKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let write_key = |f: &mut fmt::Formatter<'_>, key: &str| {
            if key.contains('.') {
                write!(f, "'{key}'")
            } else {
                write!(f, "{key}")
            }
        };

        for key in self.parents {
            write_key(f, key)?;
            write!(f, ".")?;
        }

        write_key(f, self.key)
    }
}

fn get_table(
    from: &node::Table,
    parents: &[&str],
    key: &str,
    file: &Utf8Path,
) -> Option<node::Table> {
    if let Some(table) = from.get(key) {
        if let Ok(found) = table.try_into_table() {
            Some(found)
        } else {
            let key = FullKey { parents, key };
            log::warn!("{file}: {key} is not a table");
            None
        }
    } else {
        None
    }
}
