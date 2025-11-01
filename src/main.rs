mod cli;
mod dep_collector;
mod editor;
mod managers;
mod walker;

use anyhow::Context as _;

use self::dep_collector::{DepCollector, Deps, DepsBuilder};
use self::managers::Manager;

static STATE_FILE: &str = ".updater.json";

fn main() -> Result<(), anyhow::Error> {
    init_logger();

    let cli = cli::parse()?;

    let cwd = if let Some(cwd) = cli.cwd {
        std::env::set_current_dir(&cwd).context("setting cwd")?;
        cwd
    } else {
        std::env::current_dir()
            .context("getting cwd")?
            .try_into()
            .context("converting cwd to UTF-8")?
    };

    let managers = managers::all();

    match cli.action {
        cli::Action::Usage { requested } => {
            eprintln!("{}", cli::USAGE);
            if !requested {
                std::process::exit(1);
            }
        }

        cli::Action::Init => {
            let files = walker::walk(&cwd, &managers);

            let deps = DepsBuilder::new();
            for (manager_id, paths) in files.iter().enumerate() {
                let manager = &managers[manager_id];
                for path in paths {
                    manager.scan_file(path, deps.collector(manager_id));
                }
            }

            save_state(deps.into())?;
        }

        cli::Action::Edit => {
            let mut state = load_state()?;
            editor::run(ratatui::init(), &mut state)?;
            ratatui::restore();
            save_state(state)?;
        }

        cli::Action::Summarize => {
            markdown_summary(&load_state()?);
        }

        cli::Action::Finish => match std::fs::remove_file(STATE_FILE) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        },
    }

    Ok(())
}

fn init_logger() {
    let env = env_logger::Env::new()
        .filter("UPDATER_LOG")
        .write_style("COLOR");
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();
}

fn load_state() -> anyhow::Result<Deps> {
    let raw = std::fs::read_to_string(STATE_FILE).context("reading state")?;
    let deps = Deps::deserialize(&raw)
        .map_err(facet_json::DeserError::into_owned)
        .context("deserializing state")?;
    Ok(deps)
}

fn save_state(deps: Deps) -> anyhow::Result<()> {
    std::fs::write(STATE_FILE, &deps.serialize()).context("writing state")?;
    Ok(())
}

fn markdown_summary(collector: &Deps) {
    let eprint_heading = |level, title: &_| {
        let prefix = "#".repeat(level);
        eprintln!("{prefix} {title}\n");
    };

    let mut stack = Vec::new();
    stack.push(collector.iter_root_groups().peekable());
    while let Some(iter) = stack.last_mut() {
        if let Some(group) = iter.next() {
            let mut deps = group.iter_dependencies().peekable();
            let mut subgroups = group.iter_subgroups().peekable();

            if deps.peek().is_some() || subgroups.peek().is_some() {
                let level = stack.len();
                eprint_heading(level, group.title());
            }

            let mut any_deps = false;
            for dep in deps {
                any_deps = true;
                eprintln!("- `{}`: {}", &dep.name, &dep.version);
            }

            if any_deps {
                eprintln!();
            }

            stack.push(subgroups);
        } else {
            stack.pop();
        }
    }
}
