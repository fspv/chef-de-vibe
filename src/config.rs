use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub claude_binary_path: PathBuf,
    pub http_listen_address: String,
    pub claude_projects_dir: PathBuf,
    pub shutdown_timeout: Duration,
}

impl Config {
    /// Creates a new configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if required environment variables are missing or invalid,
    /// or if the configuration validation fails.
    ///
    /// # Panics
    ///
    /// Panics if the home directory cannot be determined when `CLAUDE_PROJECTS_DIR`
    /// is not set.
    pub fn from_env() -> Result<Self> {
        let claude_binary_path = match env::var("CLAUDE_BINARY_PATH") {
            Ok(path) => {
                let path = PathBuf::from(path);
                if path.is_relative() {
                    std::fs::canonicalize(&path).with_context(|| {
                        format!("Failed to resolve relative path: {}", path.display())
                    })?
                } else {
                    path
                }
            }
            Err(_) => Self::find_claude_in_path()
                .context("CLAUDE_BINARY_PATH not set and 'claude' not found in PATH")?,
        };

        let http_listen_address =
            env::var("HTTP_LISTEN_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

        let claude_projects_dir = env::var("CLAUDE_PROJECTS_DIR").map_or_else(
            |_| {
                dirs::home_dir()
                    .expect("Could not determine home directory")
                    .join(".claude")
                    .join("projects")
            },
            PathBuf::from,
        );

        let shutdown_timeout = env::var("SHUTDOWN_TIMEOUT")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u64>()
            .context("Invalid SHUTDOWN_TIMEOUT value")?;
        let shutdown_timeout = Duration::from_secs(shutdown_timeout);

        let config = Self {
            claude_binary_path,
            http_listen_address,
            claude_projects_dir,
            shutdown_timeout,
        };

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        // Validate claude binary exists and is executable
        if !self.claude_binary_path.exists() {
            anyhow::bail!(
                "Claude binary not found at: {}",
                self.claude_binary_path.display()
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&self.claude_binary_path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                anyhow::bail!(
                    "Claude binary is not executable: {}",
                    self.claude_binary_path.display()
                );
            }
        }

        // Validate projects directory exists and is readable
        if !self.claude_projects_dir.exists() {
            anyhow::bail!(
                "Claude projects directory does not exist: {}",
                self.claude_projects_dir.display()
            );
        }

        if !self.claude_projects_dir.is_dir() {
            anyhow::bail!(
                "Claude projects directory is not a directory: {}",
                self.claude_projects_dir.display()
            );
        }

        // Test if we can read the directory
        std::fs::read_dir(&self.claude_projects_dir).with_context(|| {
            format!(
                "Cannot read Claude projects directory: {}",
                self.claude_projects_dir.display()
            )
        })?;

        Ok(())
    }

    #[must_use]
    #[allow(dead_code)] // Public API utility method
    pub fn get_project_dir(&self, working_dir: &Path) -> PathBuf {
        let safe_dir_name = working_dir.to_string_lossy().replace(['/', '\\', ':'], "_");
        self.claude_projects_dir.join(safe_dir_name)
    }

    fn find_claude_in_path() -> Result<PathBuf> {
        let path_var = env::var("PATH").unwrap_or_default();
        let paths = env::split_paths(&path_var);

        for dir in paths {
            let candidate = dir.join("claude");
            if candidate.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let metadata = std::fs::metadata(&candidate)?;
                    let permissions = metadata.permissions();
                    if permissions.mode() & 0o111 != 0 {
                        return Ok(candidate);
                    }
                }
                #[cfg(not(unix))]
                {
                    return Ok(candidate);
                }
            }
        }

        anyhow::bail!("'claude' binary not found in PATH")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn test_config_from_env_missing_required() {
        std::env::remove_var("CLAUDE_BINARY_PATH");
        std::env::remove_var("HTTP_LISTEN_ADDRESS");
        std::env::remove_var("PATH");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("CLAUDE_BINARY_PATH not set and 'claude' not found in PATH"),
            "Error was: {}",
            err_str
        );
    }

    #[test]
    #[serial]
    fn test_config_validation() {
        // Clean up any existing env vars from other tests
        env::remove_var("CLAUDE_BINARY_PATH");
        env::remove_var("HTTP_LISTEN_ADDRESS");
        env::remove_var("CLAUDE_PROJECTS_DIR");
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("claude");
        fs::write(&binary_path, "").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms).unwrap();
        }

        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        env::set_var("CLAUDE_BINARY_PATH", binary_path.to_str().unwrap());
        env::set_var("HTTP_LISTEN_ADDRESS", "127.0.0.1:8080");
        env::set_var("CLAUDE_PROJECTS_DIR", projects_dir.to_str().unwrap());

        let config = Config::from_env().unwrap();
        assert_eq!(config.http_listen_address, "127.0.0.1:8080");
        assert_eq!(config.shutdown_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_get_project_dir() {
        let config = Config {
            claude_binary_path: PathBuf::from("/usr/bin/claude"),
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: PathBuf::from("/home/user/.claude/projects"),
            shutdown_timeout: Duration::from_secs(30),
        };

        let working_dir = Path::new("/home/user/my-project");
        let project_dir = config.get_project_dir(working_dir);
        assert_eq!(
            project_dir,
            PathBuf::from("/home/user/.claude/projects/_home_user_my-project")
        );
    }

    #[test]
    fn test_find_claude_in_path() {
        // Create a temporary directory to act as a PATH location
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("claude");
        fs::write(&binary_path, "").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms).unwrap();
        }

        // Save original PATH and set our test PATH
        let original_path = env::var("PATH").ok();
        env::set_var("PATH", temp_dir.path());

        // Test finding claude in PATH
        let result = Config::find_claude_in_path();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), binary_path);

        // Restore original PATH
        if let Some(path) = original_path {
            env::set_var("PATH", path);
        } else {
            env::remove_var("PATH");
        }
    }

    #[test]
    #[serial]
    fn test_find_claude_in_path_not_found() {
        // Save original PATH and set a path that doesn't contain claude
        let original_path = env::var("PATH").ok();
        let temp_dir = TempDir::new().unwrap();
        env::set_var("PATH", temp_dir.path());

        // Test that claude is not found
        let result = Config::find_claude_in_path();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found in PATH"));

        // Restore original PATH
        if let Some(path) = original_path {
            env::set_var("PATH", path);
        } else {
            env::remove_var("PATH");
        }
    }

    #[test]
    #[serial]
    fn test_config_from_env_with_relative_path() {
        // Clean up any existing env vars
        env::remove_var("CLAUDE_BINARY_PATH");
        env::remove_var("HTTP_LISTEN_ADDRESS");
        env::remove_var("CLAUDE_PROJECTS_DIR");

        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("claude");
        fs::write(&binary_path, "").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms).unwrap();
        }

        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        // Change to temp directory so relative path works
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Set relative path
        env::set_var("CLAUDE_BINARY_PATH", "./claude");
        env::set_var("HTTP_LISTEN_ADDRESS", "127.0.0.1:8080");
        env::set_var("CLAUDE_PROJECTS_DIR", projects_dir.to_str().unwrap());

        let config = Config::from_env().unwrap();
        // The config should have canonicalized the relative path
        assert_eq!(
            config.claude_binary_path,
            binary_path.canonicalize().unwrap()
        );

        // Clean up env vars
        env::remove_var("CLAUDE_BINARY_PATH");
        env::remove_var("HTTP_LISTEN_ADDRESS");
        env::remove_var("CLAUDE_PROJECTS_DIR");

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[serial]
    fn test_config_from_env_without_claude_binary_path() {
        // Clean up any existing env vars
        env::remove_var("CLAUDE_BINARY_PATH");
        env::remove_var("HTTP_LISTEN_ADDRESS");
        env::remove_var("CLAUDE_PROJECTS_DIR");

        // Create a temporary directory to act as a PATH location
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("claude");
        fs::write(&binary_path, "").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms).unwrap();
        }

        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        // Save original PATH and set our test PATH
        let original_path = env::var("PATH").ok();
        env::set_var("PATH", temp_dir.path());

        // Don't set CLAUDE_BINARY_PATH
        env::set_var("HTTP_LISTEN_ADDRESS", "127.0.0.1:8080");
        env::set_var("CLAUDE_PROJECTS_DIR", projects_dir.to_str().unwrap());

        let config = Config::from_env().unwrap();
        // The path from find_claude_in_path should match our test binary
        assert_eq!(
            config.claude_binary_path.canonicalize().unwrap(),
            binary_path.canonicalize().unwrap()
        );

        // Clean up env vars
        env::remove_var("HTTP_LISTEN_ADDRESS");
        env::remove_var("CLAUDE_PROJECTS_DIR");

        // Restore original PATH
        if let Some(path) = original_path {
            env::set_var("PATH", path);
        } else {
            env::remove_var("PATH");
        }
    }
}
