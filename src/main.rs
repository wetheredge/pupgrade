use std::borrow::Cow;

fn main() {
    init_logger();

    let mut managers = updater::managers::all();
    let root = std::env::current_dir().unwrap();
    let files = updater::walk(&root, &managers);

    for (manager, paths) in files.iter().enumerate() {
        let manager = &mut managers[manager];
        println!("{}:", manager.name());
        for path in paths {
            println!("  {}", path.display());
            manager.scan_file(path, Scanner);
        }
    }
}

struct Scanner;

impl updater::managers::Scanner for Scanner {
    fn register(&self, _id: usize, name: &str, category: Cow<'static, str>, version: &str) {
        eprintln!("    {name}({category}): {version}");
    }
}

fn init_logger() {
    let env = env_logger::Env::new()
        .filter("UPDATER_LOG")
        .write_style("COLOR");
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();
}
