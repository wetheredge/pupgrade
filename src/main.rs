mod cli;
mod dep_collector;
mod managers;
mod walker;

use camino::Utf8PathBuf;

use self::dep_collector::DepCollector;
use self::managers::Manager;

static STATE_FILE: &str = ".updater.json";

fn main() -> Result<(), anyhow::Error> {
    init_logger();

    let cli = cli::parse()?;
    if let Some(cwd) = cli.cwd {
        std::env::set_current_dir(cwd).unwrap();
    }

    let managers = managers::all();

    match cli.action {
        cli::Action::Usage { requested } => {
            eprintln!("{}", cli::USAGE);
            if !requested {
                std::process::exit(1);
            }
        }

        cli::Action::Init => {
            let root = Utf8PathBuf::try_from(std::env::current_dir().unwrap()).unwrap();
            let files = walker::walk(&root, &managers);

            let collector = DepCollector::new();
            for (manager, paths) in files.iter().enumerate() {
                let manager = &managers[manager];
                for path in paths {
                    manager.scan_file(path, &collector);
                }
            }

            let deps = collector.serialize();
            std::fs::write(STATE_FILE, &deps)?;
        }

        cli::Action::Edit => todo!(),

        cli::Action::Summarize => {
            let raw = std::fs::read_to_string(STATE_FILE)?;
            let deps =
                DepCollector::deserialize(&raw).map_err(facet_json::DeserError::into_owned)?;

            markdown_summary(&deps);
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

fn markdown_summary(collector: &DepCollector) {
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
                group.title(|title| eprint_heading(level, title));
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
