use std::fmt;
use std::fs;
use std::io::{BufRead as _, BufReader};

use camino::Utf8Path;
use facet::Facet;
use taplo::dom::node::DomNode as _;
use taplo::dom::{Node, node};

use crate::DepCollector;
use crate::dep_collector::{Dep, DepInit, Deps, Updates, Version};

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "Cargo"
    }

    fn walk_file(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|name| name == "Cargo.toml")
    }

    fn scan_file(&self, path: &Utf8Path, collector: DepCollector<'_>) {
        let path_id = collector.push_path(path.parent().unwrap().into());

        let toml = fs::read_to_string(path).unwrap();

        let dom = taplo::parser::parse(&toml).into_dom();
        let root = dom.as_table().unwrap();

        let get_root_table = |key| get_table(root, &[], key, path);

        if let Some(workspace) = get_root_table("workspace")
            && let Some(dependencies) = get_table(&workspace, &["workspace"], "dependencies", path)
        {
            let kind_id = collector.get_kind_id("workspace".to_owned(), || "Workspace".to_owned());
            scan_inner(collector, path_id, kind_id, &dependencies);
        }

        for (key, display) in [
            ("dependencies", "Runtime"),
            ("build-dependencies", "Build"),
            ("dev-dependencies", "Dev"),
        ] {
            if let Some(table) = get_root_table(key) {
                let kind_id = collector.get_kind_id(key.to_owned(), || display.to_owned());
                scan_inner(collector, path_id, kind_id, &table);
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
            for (key, display) in [("dependencies", "Runtime"), ("build-dependencies", "Build")] {
                if let Some(dependencies) = get_table(table, &["target", target], key, path) {
                    let kind_id = collector.get_kind_id(format!("{key}\0{target}"), || {
                        format!("{display} ({target})")
                    });
                    scan_inner(collector, path_id, kind_id, &dependencies);
                }
            }
        });

        each_nested_table("patch", &mut |registry, table| {
            let kind_id = collector.get_kind_id(format!("patch\0{registry}"), || {
                format!("Patch ({registry})")
            });
            scan_inner(collector, path_id, kind_id, table);
        });
    }

    fn find_updates(&self, dep: &crate::Dep) -> Updates {
        match &dep.version {
            Version::SemVer(current) => {
                let host = "https://index.crates.io";

                // NOTE: assumes name is ascii
                let uri = match dep.name.len() {
                    0 => unreachable!(),
                    1 => format!("{host}/1/{}", &dep.name),
                    2 => format!("{host}/2/{}", &dep.name),
                    3 => format!("{host}/3/{}/{}", &dep.name[0..1], &dep.name),
                    _ => format!(
                        "{host}/{}/{}/{}",
                        &dep.name[0..2],
                        &dep.name[2..4],
                        &dep.name
                    ),
                };

                let mut response = ureq::get(uri).call().unwrap();
                let body = response.body_mut().as_reader();
                let mut crates = BufReader::new(body)
                    .lines()
                    .filter_map(|line| facet_json::from_str(&line.ok()?).ok())
                    .collect::<Vec<Crate>>();

                let Crate { vers: latest } = crates.pop().unwrap();

                let current = current.strip_prefix('=').unwrap_or(current);
                if current == latest {
                    Updates::None
                } else {
                    Updates::Found(Version::SemVer(format!("={latest}")))
                }
            }
            Version::GitCommit { .. } => todo!(),
            Version::GitPinnedTag { .. } => todo!(),
        }
    }

    fn apply(&self, deps: &Deps, dep: &Dep, version: &Version) {
        let path = deps.path(dep.path.unwrap()).join("Cargo.toml");
        let mut toml = fs::read_to_string(&path).unwrap();

        let dom = taplo::parser::parse(&toml).into_dom();
        let root = dom.as_table().unwrap();

        let table = deps.internal_kind(dep.kind.unwrap());
        let table = if let Some((head, tail)) = table.split_once('\0') {
            if head == "patch" {
                let table = get_table(root, &[], "patch", &path).unwrap();
                get_table(&table, &["patch"], tail, &path).unwrap()
            } else {
                let table = get_table(root, &[], "target", &path).unwrap();
                let table = get_table(&table, &["target"], tail, &path).unwrap();
                get_table(&table, &["target", tail], head, &path).unwrap()
            }
        } else if table == "workspace" {
            let table = get_table(root, &[], "workspace", &path).unwrap();
            get_table(&table, &["workspace"], "dependencies", &path).unwrap()
        } else {
            get_table(root, &[], table, &path).unwrap()
        };

        let name = dep.renamed.as_deref().unwrap_or(&dep.name);
        let mut replacements = Vec::new();
        let node = table.get(name).unwrap();
        match version {
            Version::SemVer(latest) => {
                let node = match node {
                    Node::Table(meta) => meta.get("version").unwrap().try_into_str().unwrap(),
                    Node::Str(current) => current,
                    _ => todo!(),
                };
                replacements.push((node.syntax().unwrap().text_range(), format!("\"{latest}\"")));
            }
            Version::GitCommit { .. } => todo!(),
            Version::GitPinnedTag { .. } => todo!(),
        }

        replacements.sort_by_key(|(range, _)| range.start());
        for (range, with) in replacements {
            let range = usize::from(range.start())..usize::from(range.end());
            toml.replace_range(range, &with);
        }

        fs::write(path, toml).unwrap();
    }
}

fn scan_inner(collector: DepCollector, path_id: usize, kind_id: usize, table: &node::Table) {
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

                    // FIXME
                    continue;
                }
            }
            Node::Str(version) => Version::SemVer(version.value().to_owned()),

            _ => todo!(),
        };

        collector.push_dep(DepInit {
            path: Some(path_id),
            kind: Some(kind_id),
            name: name.value().to_owned(),
            renamed: None,
            version,
        });
    }
}

#[derive(Facet)]
struct Crate {
    vers: String,
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
