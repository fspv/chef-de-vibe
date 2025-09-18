use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct MockClaude {
    pub temp_dir: TempDir,
    pub binary_path: PathBuf,
    pub projects_dir: PathBuf,
}

impl MockClaude {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let binary_path = temp_dir.path().join("mock_claude.py");
        let projects_dir = temp_dir.path().join("projects");

        // Copy the Python mock script to the temp directory
        let python_script = include_str!("mock_claude.py");
        fs::write(&binary_path, python_script).expect("Failed to write mock Claude script");

        // Make the script executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms).unwrap();
        }

        // Ensure file is fully written and synced to disk
        std::thread::sleep(std::time::Duration::from_millis(5));

        // Create projects directory
        fs::create_dir_all(&projects_dir).expect("Failed to create projects directory");

        MockClaude {
            temp_dir,
            binary_path,
            projects_dir,
        }
    }

    pub fn setup_env_vars(&self) {
        env::set_var("CLAUDE_BINARY_PATH", &self.binary_path);
        env::set_var("CLAUDE_PROJECTS_DIR", &self.projects_dir);
        env::set_var("HTTP_LISTEN_ADDRESS", "127.0.0.1:0"); // Use port 0 for random free port
        env::set_var("SHUTDOWN_TIMEOUT", "1");
    }

    #[allow(dead_code)] // Test helper method
    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    #[allow(dead_code)] // Test helper method
    pub fn projects_dir(&self) -> &Path {
        &self.projects_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn test_mock_claude_creation() {
        let mock = MockClaude::new();
        assert!(mock.binary_path.exists());
        assert!(mock.projects_dir.exists());
    }

    #[test]
    fn test_mock_claude_echo() {
        let mock = MockClaude::new();

        // Test that the Python script echoes JSON
        let mut child = Command::new("python3")
            .arg(&mock.binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start mock Claude");

        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        let test_json = r#"{"type": "test", "message": "hello"}"#;
        stdin
            .write_all(test_json.as_bytes())
            .expect("Failed to write to stdin");
        stdin.write_all(b"\n").expect("Failed to write newline");

        // Send exit command
        let exit_json = r#"{"control": "exit", "code": 0}"#;
        stdin
            .write_all(exit_json.as_bytes())
            .expect("Failed to write exit command");
        stdin.write_all(b"\n").expect("Failed to write newline");

        let output = child.wait_with_output().expect("Failed to read output");
        assert_eq!(output.status.code(), Some(0));

        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(test_json));
    }

    #[test]
    fn test_mock_claude_exit_control() {
        let mock = MockClaude::new();

        // Test exit with code 1
        let mut child = Command::new("python3")
            .arg(&mock.binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start mock Claude");

        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        let exit_json = r#"{"control": "exit", "code": 42}"#;
        stdin
            .write_all(exit_json.as_bytes())
            .expect("Failed to write exit command");
        stdin.write_all(b"\n").expect("Failed to write newline");

        let output = child.wait_with_output().expect("Failed to read output");
        assert_eq!(output.status.code(), Some(42));
    }

    #[test]
    fn test_mock_claude_write_file_control() {
        let mock = MockClaude::new();
        let test_file = mock.temp_dir.path().join("test_output.txt");

        // Test write_file control command
        let mut child = Command::new("python3")
            .arg(&mock.binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start mock Claude");

        let stdin = child.stdin.as_mut().expect("Failed to open stdin");

        // Send write_file command
        let write_json = format!(
            r#"{{"control": "write_file", "path": "{}", "content": "test content"}}"#,
            test_file.display()
        );
        stdin
            .write_all(write_json.as_bytes())
            .expect("Failed to write command");
        stdin.write_all(b"\n").expect("Failed to write newline");

        // Send exit command
        let exit_json = r#"{"control": "exit", "code": 0}"#;
        stdin
            .write_all(exit_json.as_bytes())
            .expect("Failed to write exit command");
        stdin.write_all(b"\n").expect("Failed to write newline");

        let _output = child.wait_with_output().expect("Failed to read output");

        // Check that file was created
        assert!(test_file.exists());
        let content = fs::read_to_string(test_file).unwrap();
        assert_eq!(content, "test content");
    }
}
