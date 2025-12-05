use std::collections::HashMap;

use camino::Utf8Path;
use facet::Facet;

use crate::DepCollector;
use crate::dep_collector::{DepInit, Version};

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "pnpm"
    }

    fn walk_file(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|name| name == "package.json")
    }

    fn scan_file(&self, path: &Utf8Path, collector: DepCollector<'_>) {
        let path_id = collector.push_path(path.parent().unwrap().into());

        let package = std::fs::read(path).unwrap();
        let package = facet_json::from_slice::<Package>(&package).unwrap();

        macro_rules! scan_inner {
            ($key:ident, $title:literal) => {
                scan_inner(
                    collector,
                    path_id,
                    collector.get_kind_id(stringify!($key).to_owned(), || $title.to_owned()),
                    package.$key,
                )
            };
            (short $key:ident, $title:literal) => {
                scan_inner(
                    collector,
                    path_id,
                    collector
                        .get_kind_id(concat!(stringify!($key), "Dependencies").to_owned(), || {
                            $title.to_owned()
                        }),
                    package.$key,
                )
            };
        }

        scan_inner!(dependencies, "Runtime");
        scan_inner!(short dev, "Dev");
        scan_inner!(short peer, "Peer");
        scan_inner!(short optional, "Optional");
        scan_inner!(overrides, "Overrides");
    }
}

fn scan_inner(collector: DepCollector<'_>, path_id: usize, kind_id: usize, deps: Deps) {
    for (mut name, mut version) in deps {
        if version == "workspace:*" {
            continue;
        }

        let mut renamed = None;
        if version.contains(':')
            && let Some((actual_name, actual_version)) = version.split_once('@')
        {
            renamed = Some(name);
            name = actual_name.to_owned();
            version = actual_version.to_owned();
        }

        collector.push_dep(DepInit {
            path: Some(path_id),
            kind: Some(kind_id),
            name,
            renamed,
            version: Version::SemVer(version),
        });
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
