#[cfg(windows)]
use std::time::{Duration, Instant};
use std::{env, fs::File, path::PathBuf};

use tauri::{Builder, Context, Wry};

use super::{DesktopOpen, receive};
#[cfg(windows)]
use crate::desktop_files::forwarded_book_paths;

pub(crate) fn register_first(
    builder: Builder<Wry>,
    context: &Context<Wry>,
) -> Result<(Builder<Wry>, Option<File>), &'static str> {
    // The official transport uses Unicode argv.
    if env::args_os().any(|arg| arg.to_str().is_none()) {
        return Ok((builder, None));
    }
    #[cfg(windows)]
    let endpoint_startup = {
        let _ = context;
        let configured = env::var_os("LOCALAPPDATA");
        let Some(profile) = dirs::data_local_dir()
            .filter(|profile| windows_uses_default_profile(configured.as_ref(), profile))
        else {
            return Ok((builder, None));
        };
        if !windows_forwardable() {
            return Ok((builder, None));
        }
        Some(lock_windows_endpoint(&profile)?)
    };
    #[cfg(target_os = "linux")]
    let dbus_id = {
        if env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none_or(|address| address.is_empty()) {
            return Ok((builder, None));
        }
        let Some(profile) = dirs::data_local_dir() else {
            return Ok((builder, None));
        };
        linux_dbus_id(&context.config().identifier, &profile)
    };
    #[cfg(target_os = "linux")]
    let endpoint_startup = None;

    let plugin = tauri_plugin_single_instance::Builder::new().callback(receive);
    #[cfg(target_os = "linux")]
    let plugin = plugin.dbus_id(dbus_id);
    Ok((
        builder
            .manage(DesktopOpen::default())
            .plugin(plugin.build()),
        endpoint_startup,
    ))
}

#[cfg(windows)]
fn windows_forwardable() -> bool {
    let argv = env::args_os().collect::<Vec<_>>();
    let cwd = env::current_dir().ok();
    windows_args_fit_transport(&argv, cwd.as_deref())
}

#[cfg(windows)]
fn windows_args_fit_transport(argv: &[std::ffi::OsString], cwd: Option<&std::path::Path>) -> bool {
    // This version uses a literal pipe delimiter. An old file:/C|/ URL
    // stays on the native cold path; never pass it through this encoder.
    let fits = |arg: &std::ffi::OsStr| arg.to_str().is_some_and(|text| !text.contains('|'));
    cwd.is_some_and(|path| fits(path.as_os_str()))
        && argv.iter().all(|arg| fits(arg))
        && (argv.len() == 1 || forwarded_book_paths(argv.iter().skip(1).cloned(), cwd).is_some())
}

#[cfg(windows)]
fn lock_windows_endpoint(profile: &std::path::Path) -> Result<File, &'static str> {
    // Serialize only Tauri builder/plugin initialization. The official
    // plugin creates its mutex before the receiving HWND in its setup hook.
    // A simultaneous launcher waits here, not in that publication gap.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(profile.join("Atha-single-instance-startup.lock"))
        .map_err(|_| "single-instance-startup-lock")?;
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return Err("single-instance-startup-lock"),
        }
    }
}

#[cfg(windows)]
fn windows_uses_default_profile(
    configured: Option<&std::ffi::OsString>,
    platform: &std::path::Path,
) -> bool {
    let (Ok(configured), Ok(platform)) = (
        configured
            .and_then(|path| std::fs::canonicalize(path).ok())
            .ok_or(()),
        std::fs::canonicalize(platform),
    ) else {
        return false;
    };
    configured == platform
}

#[cfg(target_os = "linux")]
fn linux_dbus_id(identifier: &str, profile: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};

    // Use the same data-home resolution as Tauri without changing its bundle
    // identifier, application folders or WebView origin.
    use std::fmt::Write as _;
    let profile = canonical_data_root(&profile.join(identifier));
    let mut name = format!("{identifier}.Profile.p");
    for byte in Sha256::digest(profile.as_os_str().as_encoded_bytes()) {
        write!(name, "{byte:02x}").expect("formatting a String");
    }
    name
}

#[cfg(target_os = "linux")]
fn canonical_data_root(profile: &std::path::Path) -> PathBuf {
    // Follow aliases in the deepest existing directory and append any
    // not-yet-created suffix. Startup creates the data home after IPC setup.
    for ancestor in profile.ancestors() {
        if let Ok(base) = std::fs::canonicalize(ancestor)
            && let Ok(suffix) = profile.strip_prefix(ancestor)
        {
            return suffix.components().fold(base, |mut path, segment| {
                if segment == std::path::Component::ParentDir {
                    path.pop();
                } else {
                    path.push(segment.as_os_str());
                }
                path
            });
        }
    }
    profile.components().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn ambiguous_cold_paths_do_not_enter_pipe_transport() {
        use std::{ffi::OsString, path::Path};
        let args = |path: &str| [OsString::from("atha.exe"), OsString::from(path)];
        let cwd = Some(Path::new(r"C:\Books"));
        assert!(windows_args_fit_transport(&args("书 Title.epub"), cwd));
        assert!(!windows_args_fit_transport(
            &args("file:/C|/Book.epub"),
            cwd
        ));
        assert!(!windows_args_fit_transport(&args("D:Book.epub"), cwd));
        assert!(!windows_args_fit_transport(&args("--epub"), cwd));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn separate_data_homes_have_separate_bus_names() {
        use std::path::Path;
        let first = linux_dbus_id("com.atha.reader", Path::new("/tmp/first profile"));
        let repeat = linux_dbus_id("com.atha.reader", Path::new("/tmp/first profile"));
        let second = linux_dbus_id("com.atha.reader", Path::new("/tmp/second profile"));
        assert_eq!(first, repeat);
        assert_eq!(
            first,
            linux_dbus_id("com.atha.reader", Path::new("/tmp/first profile/"))
        );
        assert_ne!(first, second);
        assert!(first.len() < 255 && !first.contains(' '));
    }
}
