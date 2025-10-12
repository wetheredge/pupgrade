fn main() {
    let managers = updater::managers::all();
    let root = std::env::current_dir().unwrap();
    let files = updater::walk(&root, &managers);
    for (manager, path) in files {
        let manager = &managers[manager as usize];
        println!("{}: {}", manager.name(), path.display());
    }
}
