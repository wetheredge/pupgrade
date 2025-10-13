fn main() {
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
