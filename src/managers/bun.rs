use std::collections::HashMap;
use std::path::{Path, PathBuf};

use facet::Facet;

use super::Scanner;

pub(super) struct Manager {
    deps: Vec<Dep>,
}

impl<S: super::Scanner> super::Manager<S> for Manager {
    fn name(&self) -> &'static str {
        "Bun"
    }

    fn filter_file(&self, path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "package.json")
    }

    fn scan_file(&mut self, file: &Path, mut scanner: S) {
        let dir = file.parent().unwrap();
        let package = std::fs::read(file).unwrap();
        let package = facet_json::from_slice::<Package>(&package).unwrap();

        self.scan_inner(dir, package.dependencies, Category::Runtime, &mut scanner);
        self.scan_inner(dir, package.dev, Category::Dev, &mut scanner);
        self.scan_inner(dir, package.peer, Category::Peer, &mut scanner);
        self.scan_inner(dir, package.optional, Category::Optional, &mut scanner);
        self.scan_inner(dir, package.overrides, Category::Override, &mut scanner);
    }
}

impl Manager {
    pub(super) fn new() -> Self {
        Self { deps: Vec::new() }
    }

    fn scan_inner(
        &mut self,
        dir: &Path,
        deps: Deps,
        category: Category,
        scanner: &mut impl Scanner,
    ) {
        for (name, version) in deps {
            if version == "workspace:*" {
                continue;
            }

            scanner.register(self.deps.len(), &name, category.name().into(), &version);
            self.deps.push(Dep {
                name,
                dir: dir.to_owned(),
            })
        }
    }
}

#[derive(Debug, Facet)]
struct Dep {
    name: String,
    dir: PathBuf,
}

#[derive(Debug, Facet, Clone, Copy, PartialEq, Eq, Hash)]
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
            Self::Runtime => "runtime",
            Self::Dev => "dev",
            Self::Peer => "peer",
            Self::Optional => "optional",
            Self::Override => "override",
        }
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
