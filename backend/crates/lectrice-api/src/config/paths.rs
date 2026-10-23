use std::path::{Path, PathBuf};

use super::ConfigError;

const HOME_MARKER: &str = "~";

/// `~/x` becomes `<home>/x`; other relative paths are taken from `base_dir`.
pub fn resolve(path: &Path, base_dir: &Path, home: Option<&Path>) -> Result<PathBuf, ConfigError> {
    if let Ok(rest) = path.strip_prefix(HOME_MARKER) {
        let home = home.ok_or_else(|| ConfigError::NoHome(path.to_path_buf()))?;
        return Ok(home.join(rest));
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(absolute(base_dir).join(path))
}

fn absolute(dir: &Path) -> PathBuf {
    std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home() {
        let resolved = resolve(Path::new("~/.local/share/lectrice"), Path::new("/app"), Some(Path::new("/home/reader"))).unwrap();
        assert_eq!(resolved, Path::new("/home/reader/.local/share/lectrice"));
    }

    #[test]
    fn home_is_required_for_tilde() {
        assert!(matches!(resolve(Path::new("~/x"), Path::new("/app"), None), Err(ConfigError::NoHome(_))));
    }

    #[test]
    fn relative_paths_follow_the_config_file() {
        assert_eq!(resolve(Path::new("vendor/lib.so"), Path::new("/app"), None).unwrap(), Path::new("/app/vendor/lib.so"));
        assert_eq!(resolve(Path::new("/opt/lib.so"), Path::new("/app"), None).unwrap(), Path::new("/opt/lib.so"));
    }

    #[test]
    fn tilde_inside_a_name_is_left_alone() {
        assert_eq!(resolve(Path::new("~books"), Path::new("/app"), None).unwrap(), Path::new("/app/~books"));
    }
}
