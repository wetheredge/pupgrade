mod managers;
mod summary;
mod walker;

use self::managers::Manager;

fn main() {
    init_logger();

    let mut managers = managers::all();
    let root = std::env::current_dir().unwrap();
    let files = walker::walk(&root, &managers);

    let summary_context = managers::SummaryContext { root };

    let mut summary = Vec::new();
    for (manager, paths) in files.iter().enumerate() {
        let manager = &mut managers[manager];
        for path in paths {
            manager.scan_file(path);
        }

        summary.push(summary::Heading {
            name: manager.name().into(),
            contents: Box::new(manager.summary(&summary_context)),
        });
    }

    let summary = summary::Node::Headings(summary.into_boxed_slice());
    println!("{}", summary.display());
}

fn init_logger() {
    let env = env_logger::Env::new()
        .filter("UPDATER_LOG")
        .write_style("COLOR");
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();
}
