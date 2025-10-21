use std::path::{Path, PathBuf};

use crate::{managers::utils::get_or_insert, summary};

#[derive(Debug)]
pub(super) struct BasicDep<C, V = String> {
    pub(super) file: PathBuf,
    pub(super) category: C,
    #[expect(unused)]
    pub(super) alias: Option<String>,
    pub(super) name: String,
    pub(super) version: V,
}

pub(super) fn summary<C: Clone + Ord + ToString, V: Ord + ToString>(
    deps: &[BasicDep<C, V>],
    context: &super::SummaryContext,
) -> summary::Node {
    let mut files = Vec::<(&Path, Vec<(C, Vec<(&str, &V, summary::Paragraph)>)>)>::new();
    for dep in deps {
        let file = get_or_insert(&mut files, &&*dep.file, Vec::new);
        let category = get_or_insert(file, &dep.category, Vec::new);

        let i = category
            .binary_search_by_key(&(dep.name.as_str(), &dep.version), |(name, version, _)| {
                (name, version)
            })
            .expect_err("dependency name & version pairs are unique");
        let paragraph =
            summary::paragraph!("`" {dep.name.to_owned()} "`: `" {dep.version.to_string()} "`");
        category.insert(i, (dep.name.as_str(), &dep.version, paragraph));
    }

    let files = files
        .into_iter()
        .map(|(file, categories)| {
            let file = file.strip_prefix(&context.root).unwrap();
            let name = format!("`{}`", file.display());

            let categories = categories
                .into_iter()
                .map(|(category, deps)| {
                    let deps = deps
                        .into_iter()
                        .map(|(_, _, dep)| summary::ListItem {
                            contents: dep,
                            sublist: Box::new([]),
                        })
                        .collect::<Vec<_>>();

                    summary::Heading {
                        name: category.to_string().into(),
                        contents: Box::new(summary::Node::List(deps.into_boxed_slice())),
                    }
                })
                .collect::<Vec<_>>();

            summary::Heading {
                name: name.into(),
                contents: Box::new(summary::Node::Headings(categories.into_boxed_slice())),
            }
        })
        .collect::<Vec<_>>();

    summary::Node::Headings(files.into_boxed_slice())
}
