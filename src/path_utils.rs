use std::path::{Path, PathBuf};

/// Strip Windows verbatim/extended-length path prefixes so the resulting path
/// is usable as a shell working directory and can be displayed to users.
///
/// On non-Windows platforms this is a no-op.
#[cfg(not(target_os = "windows"))]
pub fn normalize_windows_verbatim_path_for_shell(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Strip Windows verbatim/extended-length path prefixes so the resulting path
/// is usable as a shell working directory and can be displayed to users.
///
/// Handles:
/// - `\\?\C:\foo` → `C:\foo`
/// - `\\?\UNC\server\share\foo` → `\\server\share\foo`
/// - Everything else passes through unchanged.
#[cfg(target_os = "windows")]
pub fn normalize_windows_verbatim_path_for_shell(path: &Path) -> PathBuf {
    let path_str = path.as_os_str().to_string_lossy();

    // Verbatim disk path: \\?\C:\...
    if let Some(rest) = path_str.strip_prefix(r"\\?\") {
        if rest.len() >= 2 && rest.as_bytes()[1] == b':' {
            // Looks like a drive letter path (e.g. C:\...)
            return PathBuf::from(rest);
        }
        if let Some(unc_rest) = rest.strip_prefix(r"UNC\") {
            // Verbatim UNC path: \\?\UNC\server\share\...
            // Convert to standard UNC: \\server\share\...
            return PathBuf::from(format!(r"\\{}", unc_rest));
        }
    }

    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[cfg(target_os = "windows")]
    fn verbatim_disk_path_stripped() {
        assert_eq!(
            normalize_windows_verbatim_path_for_shell(Path::new(r"\\?\C:\Users\foo")),
            PathBuf::from(r"C:\Users\foo")
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn verbatim_unc_path_converted() {
        assert_eq!(
            normalize_windows_verbatim_path_for_shell(Path::new(r"\\?\UNC\server\share\dir")),
            PathBuf::from(r"\\server\share\dir")
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn normal_disk_path_unchanged() {
        assert_eq!(
            normalize_windows_verbatim_path_for_shell(Path::new(r"C:\Users\foo")),
            PathBuf::from(r"C:\Users\foo")
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn normal_unc_path_unchanged() {
        assert_eq!(
            normalize_windows_verbatim_path_for_shell(Path::new(r"\\server\share\dir")),
            PathBuf::from(r"\\server\share\dir")
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn non_verbatim_prefix_unchanged() {
        // A path that starts with \\ but is not a verbatim prefix
        assert_eq!(
            normalize_windows_verbatim_path_for_shell(Path::new(r"\\.\pipe\foo")),
            PathBuf::from(r"\\.\pipe\foo")
        );
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn non_windows_platform_no_op() {
        assert_eq!(
            normalize_windows_verbatim_path_for_shell(Path::new("/home/user/repo")),
            PathBuf::from("/home/user/repo")
        );
    }
}
