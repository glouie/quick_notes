//! Alternate binary name (`qn`) that forwards to the `quick_notes` library.
//! Keeping the alias as a real binary avoids shell alias requirements.

fn main() {
    if let Err(err) = quick_notes::entry() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
