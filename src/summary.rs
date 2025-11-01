use std::io::{self, Write};

use crate::dep_collector::{Dep, Deps, Updates};

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

        write!(
            out,
            "| {status} | `{name}` | {version:#} | ",
            name = &row.name,
            version = &row.version,
        )?;

        match &row.updates {
            Updates::None => {}
            Updates::Failed => write!(out, "*failed*")?,
            Updates::Found(version) => write!(out, "{version:#}")?,
        }

        let has_path = row.path.is_some();
        writeln!(
            out,
            " | {manager} | {p_tick}{p_slash}{path}{p_tick} | {kind} |",
            manager = managers[row.manager].name(),
            path = get_path_str(row),
            p_tick = if has_path { "`" } else { "" },
            p_slash = if has_path { "/" } else { "" },
            kind = get_kind_str(row),
        )?;
    }

    Ok(())
}
