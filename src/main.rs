//! Binary entrypoint for `quick_notes`.
//! Delegates all logic to the `quick_notes` library so both `quick_notes` and
//! the `qn` symlink share the same behavior.

fn main() {
    if let Err(err) = quick_notes::entry() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
