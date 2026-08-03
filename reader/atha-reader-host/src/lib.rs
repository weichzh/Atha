#[cfg(windows)]
#[path = "windows/diagnostics.rs"]
pub mod diagnostics;

#[cfg(windows)]
#[path = "windows/launch.rs"]
pub mod launch;

#[cfg(windows)]
#[path = "windows.rs"]
mod windows;

#[cfg(windows)]
pub use windows::run;
