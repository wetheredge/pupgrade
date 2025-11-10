use std::collections::HashMap;

use camino::Utf8Path;
use facet::Facet;

use crate::dep_collector::{GroupHandle, Version};

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "Bun"
    }

    fn walk_file(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|name| name == "package.json")
    }

    fn scan_file(&self, path: &Utf8Path, collector: crate::DepCollector<'_>) {
        let root = collector
            .get_or_push_group("bun".into(), || "Bun".to_owned())
            .unwrap();
        let path_string = path.as_str().to_owned();
        let group = root.new_subgroup(path_string.clone(), path_string).unwrap();

        let package = std::fs::read(path).unwrap();
        let package = facet_json::from_slice::<Package>(&package).unwrap();

        macro_rules! scan_inner {
            ($key:ident, $title:literal) => {
                scan_inner(
                    path,
                    &group
                        .new_subgroup(stringify!($key).to_owned(), $title.to_owned())
                        .unwrap(),
                    package.$key,
                )
            };
            (short $key:ident, $title:literal) => {
                scan_inner(
                    path,
                    &group
                        .new_subgroup(
                            concat!(stringify!($key), "Dependencies").to_owned(),
                            $title.to_owned(),
                        )
                        .unwrap(),
                    package.$key,
                )
            };
        }

        scan_inner!(dependencies, "Runtime");
        scan_inner!(short dev, "Development");
        scan_inner!(short peer, "Peer");
        scan_inner!(short optional, "Optional");
        scan_inner!(overrides, "Overrides");
    }
}

fn scan_inner(file: &Utf8Path, group: &GroupHandle, deps: Deps) {
    for (mut name, mut version) in deps {
        if version == "workspace:*" {
            continue;
        }

        let mut renamed = None;
        if let Some(rest) = version.strip_prefix("npm:") {
            if let Some((actual_name, actual_version)) = rest.split_once('@') {
                renamed = Some(name);
                name = actual_name.to_owned();
                version = actual_version.to_owned();
            } else {
                let group_id = group.full_id(|id| id.join("."));
                log::warn!(
                    "{file}: {group_id} dependency {name} looks like an override but has no @"
                );
                continue;
            }
        }

        group.push_dep(name, renamed, Version::SemVer(version));
    }
}

#[derive(Debug, Facet)]
struct Package {
    name: String,
    #[facet(default)]
    dependencies: Deps,
    #[facet(default, rename = "devDependencies")]
    dev: Deps,
    #[facet(default, rename = "peerDependencies")]
    peer: Deps,
    #[facet(default, rename = "optionalDependencies")]
    optional: Deps,
    #[facet(default)]
    overrides: Deps,
}

type Deps = HashMap<String, String>;
