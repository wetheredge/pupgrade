mod cli;
mod dep_collector;
mod editor;
mod managers;
mod summary;
mod walker;

use std::io::{self, BufWriter};

use anyhow::Context as _;

use crate::dep_collector::Updates;

use self::dep_collector::{Dep, DepCollector, Deps, DepsBuilder};
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

            log::info!("Found {} dependencies", deps.count());

            let mut deps = Deps::from(deps);
            for dep in deps.deps_mut() {
                log::info!("Finding updates for {}", &dep.name);
                dep.updates = managers[dep.manager].find_updates(dep);
            }

            save_state(deps)?;
        }

        cli::Action::Edit => {
            let mut state = load_state()?;
            editor::run(&mut state)?;
            save_state(state)?;
        }

        cli::Action::Apply => {
            let state = load_state()?;
            for dep in state.deps() {
                if !dep.skip
                    && let Updates::Found(version) = &dep.updates
                {
                    managers[dep.manager].apply(&state, dep, version);
                }
            }
        }

        cli::Action::Summarize => {
            let stderr = io::stderr().lock();
            summary::write_markdown(&load_state()?, &mut BufWriter::new(stderr))?;
        }

        cli::Action::Clean => match std::fs::remove_file(STATE_FILE) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
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
    let deps = Deps::deserialize(&raw).context("deserializing state")?;
    Ok(deps)
}

fn save_state(deps: Deps) -> anyhow::Result<()> {
    std::fs::write(STATE_FILE, deps.serialize()).context("writing state")?;
    Ok(())
}
