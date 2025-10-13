fn main() {
    init_logger();

    let managers = updater::managers::all();
    let root = std::env::current_dir().unwrap();
    let files = updater::walk(&root, &managers);
    for (manager, paths) in files {
        println!("{}:", manager.name());
        for path in paths {
            println!("  {}", path.display());
        }
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
