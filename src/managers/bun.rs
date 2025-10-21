use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use facet::Facet;

use crate::summary;

pub(super) struct Manager {
    deps: Vec<Dep>,
}

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "Bun"
    }

    fn filter_file(&self, path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "package.json")
    }

    fn scan_file(&mut self, file: &Path) {
        let package = std::fs::read(file).unwrap();
        let package = facet_json::from_slice::<Package>(&package).unwrap();

        self.scan_inner(file, package.dependencies, Category::Runtime);
        self.scan_inner(file, package.dev, Category::Dev);
        self.scan_inner(file, package.peer, Category::Peer);
        self.scan_inner(file, package.optional, Category::Optional);
        self.scan_inner(file, package.overrides, Category::Override);
    }

    fn summary(&self, context: &super::SummaryContext) -> summary::Node {
        super::basic_dep::summary(&self.deps, context)
    }
}

impl Manager {
    pub(super) fn new() -> Self {
        Self { deps: Vec::new() }
    }

    fn scan_inner(&mut self, file: &Path, deps: Deps, category: Category) {
        for (mut name, mut version) in deps {
            if version == "workspace:*" {
                continue;
            }

            let mut alias = None;
            if let Some(rest) = version.strip_prefix("npm:") {
                if let Some((actual_name, actual_version)) = rest.split_once('@') {
                    alias = Some(name);
                    name = actual_name.to_owned();
                    version = actual_version.to_owned();
                } else {
                    log::warn!(
                        "{}: {category:?} dependency {name} looks like an override, but has no @",
                        file.display()
                    );
                    continue;
                }
            }

            self.deps.push(Dep {
                file: file.to_owned(),
                category,
                alias,
                name,
                version,
            })
        }
    }
}

type Dep = super::BasicDep<Category>;

#[derive(Debug, Facet, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
enum Category {
    Runtime,
    Dev,
    Peer,
    Optional,
    Override,
}

impl Category {
    const fn name(&self) -> &'static str {
        match self {
            Self::Runtime => "Runtime",
            Self::Dev => "Development",
            Self::Peer => "Peer",
            Self::Optional => "Optional",
            Self::Override => "Override",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Facet)]
struct Package {
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
