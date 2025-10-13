use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

use taplo::dom::Node;
use taplo::dom::node::{self, DomNode};

use super::{Scanner, Spanned};

pub(super) struct Manager {
    deps: Vec<Dep>,
}

impl<S: super::Scanner> super::Manager<S> for Manager {
    fn name(&self) -> &'static str {
        "Cargo"
    }

    fn filter_file(&self, path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "Cargo.toml")
    }

    fn scan_file(&mut self, file: &Path, mut scanner: S) {
        let toml = std::fs::read_to_string(file).unwrap();

        let dom = taplo::parser::parse(&toml).into_dom();
        let root = dom.as_table().unwrap();

        let get_root_table = |key| get_table(root, &[], key, file);

        if let Some(workspace) = get_root_table("workspace")
            && let Some(dependencies) = get_table(&workspace, &["workspace"], "dependencies", file)
        {
            self.scan_inner(file, &dependencies, Category::Workspace, &mut scanner);
        }

        let tables = &[
            (Category::Runtime(None), "dependencies"),
            (Category::Build(None), "build-dependencies"),
            (Category::Dev, "dev-dependencies"),
        ];
        for (category, key) in tables {
            if let Some(table) = get_root_table(*key) {
                self.scan_inner(file, &table, category.clone(), &mut scanner)
            }
        }

        let each_nested_table = |root, run: &mut dyn FnMut(&str, &node::Table)| {
            if let Some(parent) = get_root_table(root) {
                for (child, table) in parent.entries().read().iter() {
                    let child = child.value();
                    if let Some(table) = table.as_table() {
                        run(child, table);
                    } else {
                        log::warn!("{}: {root}.'{child}' is not a table", file.display())
                    }
                }
            }
        };

        each_nested_table("target", &mut |target, table| {
            let tables = &[
                (Category::Runtime(Some(target.to_owned())), "dependencies"),
                (
                    Category::Build(Some(target.to_owned())),
                    "build-dependencies",
                ),
            ];
            for (category, key) in tables {
                if let Some(dependencies) = get_table(table, &["target", target], key, file) {
                    self.scan_inner(file, &dependencies, category.clone(), &mut scanner)
                }
            }
        });

        each_nested_table("patch", &mut |registry, table| {
            let category = Category::Patch {
                registry: registry.to_owned(),
            };
            self.scan_inner(file, table, category, &mut scanner);
        });
    }
}

impl Manager {
    pub(super) fn new() -> Self {
        Self { deps: Vec::new() }
    }

    fn scan_inner(
        &mut self,
        file: &Path,
        table: &node::Table,
        category: Category,
        scanner: &mut impl Scanner,
    ) {
        for (name, meta) in table.entries().read().iter() {
            let version = match meta {
                Node::Table(meta) => {
                    if let Some(version) = meta.get("version") {
                        let version = spanned_str_node(version.as_str().unwrap());
                        Version::SemVer(version)
                    } else if let Some(repo) = meta.get("git") {
                        let repo = spanned_str_node(repo.as_str().unwrap());
                        let revision = meta.get("rev").as_ref().map(spanned_node_as_str);
                        let tag = meta.get("tag").as_ref().map(spanned_node_as_str);
                        Version::Git {
                            repo,
                            revision,
                            tag,
                        }
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
                Node::Str(version) => Version::SemVer(spanned_str_node(version)),

                _ => todo!(),
            };

            scanner.register(
                self.deps.len(),
                name.value(),
                category.name(),
                version.to_pretty(),
            );

            self.deps.push(Dep {
                name: name.value().to_owned(),
                file: file.to_owned(),
                version,
            });
        }
    }
}

#[derive(Debug)]
#[expect(unused)]
struct Dep {
    name: String,
    file: PathBuf,
    version: Version,
}

#[derive(Debug, Clone)]
enum Category {
    Workspace,
    Runtime(Option<String>),
    Build(Option<String>),
    Dev,
    Patch { registry: String },
}

impl Category {
    fn name(&self) -> Cow<'static, str> {
        match self {
            Self::Workspace => "workspace".into(),
            Self::Runtime(Some(target)) => format!("runtime:{target}").into(),
            Self::Runtime(None) => "runtime".into(),
            Self::Build(Some(target)) => format!("build:{target}").into(),
            Self::Build(None) => "build".into(),
            Self::Dev => "peer".into(),
            Self::Patch { registry } => format!("patch:{registry}").into(),
        }
    }
}

#[derive(Debug)]
enum Version {
    SemVer(Spanned<String>),
    Git {
        repo: Spanned<String>,
        revision: Option<Spanned<String>>,
        tag: Option<Spanned<String>>,
    },
}

impl Version {
    fn to_pretty(&self) -> &str {
        match self {
            Self::SemVer(version) => &version.value,
            Self::Git {
                repo,
                revision,
                tag,
            } => match (tag, revision) {
                (Some(tag), _) => &tag.value,
                (_, Some(revision)) => &revision.value,
                (None, None) => &repo.value,
            },
        }
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

fn get_table(from: &node::Table, parents: &[&str], key: &str, file: &Path) -> Option<node::Table> {
    if let Some(table) = from.get(key) {
        if let Ok(found) = table.try_into_table() {
            Some(found)
        } else {
            let key = FullKey { parents, key };
            log::warn!("{}: {} is not a table", file.display(), key);
            None
        }
    } else {
        None
    }
}

fn spanned_node_as_str(node: &Node) -> Spanned<String> {
    spanned_str_node(node.as_str().unwrap())
}

fn spanned_str_node(node: &node::Str) -> Spanned<String> {
    let span = node.syntax().unwrap().text_range();
    Spanned {
        value: node.value().to_owned(),
        span: span.start().into()..span.end().into(),
    }
}
