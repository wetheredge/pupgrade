mod dep_collector;
mod managers;
mod walker;

use camino::Utf8PathBuf;

use self::dep_collector::DepCollector;
use self::managers::Manager;

fn main() {
    init_logger();

    let mut managers = managers::all();
    let root = Utf8PathBuf::try_from(std::env::current_dir().unwrap()).unwrap();
    let files = walker::walk(&root, &managers);

    let collector = DepCollector::new();
    for (manager, paths) in files.iter().enumerate() {
        let manager = &mut managers[manager];
        for path in paths {
            manager.scan_file(path, &collector);
        }
    }

    markdown_summary(&collector);
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

    for root in collector.iter_root_groups() {
        root.title(|title| eprint_heading(1, title));

        let mut stack = Vec::new();
        stack.push(root.iter_subgroups().peekable());
        while let Some(iter) = stack.last_mut() {
            if let Some(group) = iter.next() {
                let mut deps = group.iter_dependencies().peekable();
                let mut subgroups = group.iter_subgroups().peekable();

                if deps.peek().is_some() || subgroups.peek().is_some() {
                    let level = 1 + stack.len();
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
}
