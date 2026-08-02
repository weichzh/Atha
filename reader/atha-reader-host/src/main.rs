#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("atha-reader-host requires Windows");
    std::process::exit(1);
}
