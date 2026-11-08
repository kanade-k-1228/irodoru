//! Retain OS file-open requests until the webview is ready to receive them.
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
use std::ffi::OsString;

#[derive(Default)]
pub struct ProjectOpenRequests(Mutex<Vec<PathBuf>>);

impl ProjectOpenRequests {
    #[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
    pub fn push(&self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut pending = self.0.lock().unwrap_or_else(|e| e.into_inner());
        pending.extend(paths.into_iter().filter(|path| is_project(path)));
    }

    pub fn take(&self) -> Vec<PathBuf> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

pub fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("iro"))
}

/// The single-instance plugin includes argv[0] and the sender's working directory.
#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
pub fn argument_paths(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter_map(|arg| {
            if arg.to_string_lossy().starts_with('-') || arg.to_string_lossy().contains("://") {
                return None;
            }
            let path = PathBuf::from(arg);
            if !is_project(&path) {
                return None;
            }
            Some(if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_preserve_native_paths_and_resolve_the_sending_directory() {
        let dir = tempfile::tempdir().unwrap();
        let absolute = dir.path().join("絶対 パス.iro");
        let args = [
            OsString::from("app.iro"),
            OsString::from("--flag.iro"),
            OsString::from("old.zip"),
            OsString::from("https://example.com/project.iro"),
            OsString::from("file:///tmp/project.iro"),
            OsString::from("相対 パス.IRO"),
            absolute.clone().into_os_string(),
        ];
        assert_eq!(
            argument_paths(args, dir.path()),
            vec![dir.path().join("相対 パス.IRO"), absolute]
        );
    }

    #[test]
    fn startup_and_running_requests_wait_in_order_until_taken() {
        let requests = ProjectOpenRequests::default();
        requests.push(["first.iro".into(), "ignored.zip".into()]);
        requests.push(["second.IRO".into()]);
        assert_eq!(
            requests.take(),
            vec![PathBuf::from("first.iro"), PathBuf::from("second.IRO")]
        );
        assert!(requests.take().is_empty());
    }
}
