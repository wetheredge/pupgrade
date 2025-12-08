use std::collections::HashMap;
use std::fs::File;

use camino::Utf8Path;
use facet::Facet;

use crate::DepCollector;
use crate::dep_collector::{Dep, DepInit, Deps, Updates, Version};

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

    fn find_updates(&self, dep: &crate::Dep) -> Updates {
        match &dep.version {
            Version::SemVer(current) => {
                let data = ureq::get(format!("https://registry.npmjs.org/{}/latest", &dep.name))
                    .call()
                    .unwrap()
                    .into_body()
                    .read_to_vec()
                    .unwrap();
                let RegistryData { version } = facet_json::from_slice(&data).unwrap();
                if current == &version {
                    Updates::None
                } else {
                    Updates::Found(Version::SemVer(version))
                }
            }
            Version::GitCommit { .. } => todo!(),
            Version::GitPinnedTag { .. } => todo!(),
        }
    }

    fn apply(&self, deps: &Deps, dep: &Dep, version: &Version) {
        let path = deps.path(dep.path.unwrap()).join("package.json");

        let Version::SemVer(latest) = version else {
            unreachable!()
        };

        let json = std::fs::read(&path).unwrap();
        let mut json: serde_json::Value = serde_json::from_slice(&json).unwrap();

        let kind = deps.internal_kind(dep.kind.unwrap());
        let name = dep.renamed.as_deref().unwrap_or(&dep.name);
        json.get_mut(kind)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(name.to_owned(), serde_json::Value::String(latest.clone()));

        let writer = File::create(path).unwrap();
        serde_json::to_writer_pretty(writer, &json).unwrap();
    }
}

fn scan_inner(collector: DepCollector<'_>, path_id: usize, kind_id: usize, deps: PackageDeps) {
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
    dependencies: PackageDeps,
    #[facet(default, rename = "devDependencies")]
    dev: PackageDeps,
    #[facet(default, rename = "peerDependencies")]
    peer: PackageDeps,
    #[facet(default, rename = "optionalDependencies")]
    optional: PackageDeps,
    #[facet(default)]
    overrides: PackageDeps,
}

type PackageDeps = HashMap<String, String>;

#[derive(Debug, Facet)]
struct RegistryData {
    version: String,
}
