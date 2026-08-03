#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    atha_reader_host::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("atha-reader-host requires Windows");
    std::process::exit(1);
}
