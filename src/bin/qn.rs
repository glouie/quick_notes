fn main() {
    if let Err(err) = quick_notes::entry() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
