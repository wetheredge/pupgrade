use std::io::{self, Write};

use crate::dep_collector::{Dep, Deps, Version};

pub(crate) fn write_markdown(collector: &Deps, out: &mut impl Write) -> io::Result<()> {
    let managers = crate::managers::all();
    let mut table = collector.deps().iter().collect::<Vec<_>>();

    let get_path_str = |dep: &Dep| {
        dep.path
            .map(|id| collector.path(id).as_str())
            .unwrap_or_default()
    };
    let get_kind_str = |dep: &Dep| dep.kind.map(|id| collector.kind(id)).unwrap_or_default();

    table.sort_by_cached_key(|dep| {
        (
            managers[dep.manager].name(),
            &dep.name,
            dep.renamed.as_deref().unwrap_or_default(),
            get_path_str(dep),
            get_kind_str(dep),
        )
    });

    writeln!(out, "|    | Name | Old | New | Manager | Path | Kind |")?;
    writeln!(out, "|:---|:-----|:----|:----|:--------|:-----|:-----|")?;
    for row in table {
        let status = if row.skip {
            "❌"
        } else if row.updates.is_none() {
            "  "
        } else {
            "✔️"
        };

        write!(out, "| {status} | `{name}` | ", name = &row.name,)?;

        let detail = row
            .updates
            .as_ref()
            .and_then(|new| {
                fn differs(
                    f: impl Fn(&Version) -> Option<&str>,
                    old: &Version,
                    new: &Version,
                ) -> bool {
                    f(old).zip(f(new)).is_some_and(|(old, new)| old != new)
                }
                let old = &row.version;
                if differs(Version::repo, old, new) {
                    Some(Detail::Repo)
                } else if differs(Version::commit, old, new) {
                    Some(Detail::Commit)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        write_version(out, &row.version, detail)?;
        write!(out, " | ")?;
        if let Some(new) = row.updates.as_ref() {
            write_version(out, new, detail)?;
        }

        writeln!(
            out,
            " | {manager} | {path} | {kind} |",
            manager = managers[row.manager].name(),
            path = get_path_str(row),
            kind = get_kind_str(row),
        )?;
    }

    Ok(())
}

fn short_hash(commit: &str) -> &str {
    if commit.is_ascii() {
        &commit[0..commit.len().min(8)]
    } else {
        commit
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Detail {
    #[default]
    Tag,
    Commit,
    Repo,
}

fn write_version(out: &mut impl Write, version: &Version, detail: Detail) -> io::Result<()> {
    match version {
        Version::SemVer(version) => write!(out, "{version}"),
        Version::GitCommit { repo, commit } => {
            write!(out, "`{}`", short_hash(commit))?;
            if detail == Detail::Repo {
                write!(out, " ({repo})")?;
            }
            Ok(())
        }
        Version::GitPinnedTag { repo, commit, tag } => {
            write!(out, "{tag}")?;
            if detail >= Detail::Commit {
                write!(out, "@`{}`", short_hash(commit))?;
                if detail == Detail::Repo {
                    write!(out, " ({repo})")?;
                }
            }
            Ok(())
        }
    }
}
