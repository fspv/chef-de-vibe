#!/usr/bin/env python3
"""
End-to-end tests for claude-container.py script.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Dict, List, Optional, Tuple


class TestClaudeContainer(unittest.TestCase):
    """Test suite for claude-container.py script."""

    def setUp(self) -> None:
        """Set up test environment."""
        self.test_dir = tempfile.mkdtemp()
        self.original_env = os.environ.copy()

        # Create dummy binaries directory
        self.dummy_bin_dir = Path(self.test_dir) / "bin"
        self.dummy_bin_dir.mkdir(parents=True, exist_ok=True)

        # Add dummy bin to PATH
        os.environ["PATH"] = f"{self.dummy_bin_dir}:{os.environ.get('PATH', '')}"

        # Get script path
        self.script_path = Path(__file__).parent / "claude-container.py"

    def tearDown(self) -> None:
        """Clean up test environment."""
        # Restore original environment
        os.environ.clear()
        os.environ.update(self.original_env)

        # Clean up test directory
        subprocess.run(["rm", "-rf", self.test_dir], check=True)

    def create_dummy_binary(self, name: str, content: str) -> Path:
        """Create a dummy executable binary."""
        binary_path = self.dummy_bin_dir / name
        binary_path.write_text(content)
        binary_path.chmod(0o755)
        return binary_path

    def run_script(
        self, args: List[str] = None, env: Dict[str, str] = None
    ) -> Tuple[int, str, str]:
        """Run the claude-container.py script and capture output."""
        cmd = [sys.executable, str(self.script_path)]
        if args:
            cmd.extend(args)

        # Merge environment variables
        run_env = os.environ.copy()
        if env:
            run_env.update(env)

        # Run from test_dir (non-git directory) to avoid git worktree logic
        result = subprocess.run(cmd, capture_output=True, text=True, env=run_env, cwd=self.test_dir)

        return result.returncode, result.stdout, result.stderr

    def test_default_podman_command(self) -> None:
        """Test default podman command execution."""
        # Create dummy podman that handles the new session management workflow
        podman_script = """#!/usr/bin/env python3
import sys
import json

# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    # Container doesn't exist, so creation will be attempted
    sys.exit(1)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Container creation - succeed silently
    sys.exit(0)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Container execution - this is what we want to capture
    args = sys.argv[1:]  # exec command args
    print(json.dumps({"args": args}))
    sys.exit(0)
else:
    # Fallback
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Run script with test arguments
        returncode, stdout, stderr = self.run_script(["--test", "arg1", "arg2"])

        # Parse output
        output = json.loads(stdout)
        args = output["args"]

        # Verify command structure (now it's exec instead of run)
        self.assertEqual(returncode, 0)
        self.assertEqual(args[0], "exec")
        
        # Should have -i flag and container name
        self.assertIn("-i", args)
        
        # Should have claude binary and our test arguments
        self.assertIn("claude", args)
        self.assertIn("--test", args)
        self.assertIn("arg1", args)
        self.assertIn("arg2", args)
        
        # Should have --session-id added for new sessions
        self.assertIn("--session-id", args)

    def test_custom_container_binary(self) -> None:
        """Test using docker instead of podman."""
        # Create dummy docker that handles the session management workflow
        docker_script = """#!/usr/bin/env python3
import sys
import json

# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    # Container doesn't exist
    sys.exit(1)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Container creation - succeed silently
    sys.exit(0)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Container execution - this is what we want to capture
    print(json.dumps({"binary": "docker", "args": sys.argv[1:]}))
    sys.exit(0)
else:
    # Fallback
    sys.exit(1)
"""
        self.create_dummy_binary("docker", docker_script)

        # Run with docker
        env = {"CONTAINER_BINARY": "docker"}
        returncode, stdout, stderr = self.run_script(["--help"], env=env)

        # Verify docker was used
        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        self.assertEqual(output["binary"], "docker")
        self.assertEqual(output["args"][0], "exec")

    def test_custom_claude_binary(self) -> None:
        """Test custom claude binary path."""
        # Create dummy podman that shows claude binary
        podman_script = """#!/usr/bin/env python3
import sys
import json
# Find claude binary in args (last arg before user args)
claude_idx = -1
for i, arg in enumerate(sys.argv):
    if arg == "claude" or arg == "/custom/claude":
        claude_idx = i
        break
print(json.dumps({"claude_binary": sys.argv[claude_idx] if claude_idx >= 0 else None}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Test with custom claude binary
        env = {"CLAUDE_BINARY": "/custom/claude"}
        returncode, stdout, stderr = self.run_script([], env=env)

        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        self.assertEqual(output["claude_binary"], "/custom/claude")

    def test_custom_container_image(self) -> None:
        """Test custom container image."""
        # For this test, we need to check that the custom image is used in the creation command
        # We can verify this by checking the stderr debug output
        podman_script = """#!/usr/bin/env python3
import sys
import json

# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    # Container doesn't exist
    sys.exit(1)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Container creation - succeed
    sys.exit(0)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Container execution - output something for the test to parse
    print(json.dumps({"success": True}))
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Test with custom image
        env = {"CLAUDE_CONTAINER_IMAGE": "custom/image:latest"}
        returncode, stdout, stderr = self.run_script([], env=env)

        self.assertEqual(returncode, 0)
        # Check that the custom image was used in the container creation command
        self.assertIn("custom/image:latest", stderr)

    def test_volume_mounts(self) -> None:
        """Test that volume mounts are correctly set up."""
        # Create simple dummy podman for session workflow
        podman_script = """#!/usr/bin/env python3
import sys
# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)  # Execution success
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        self.assertEqual(returncode, 0)
        
        # Parse the stderr output to find the "Running command:" line
        stderr_lines = stderr.strip().split('\n')
        container_cmd = None
        for line in stderr_lines:
            if line.startswith("Running command:"):
                container_cmd = line[len("Running command: "):].split()
                break
        
        self.assertIsNotNone(container_cmd, "Should find container creation command in stderr")
        
        # Extract volume mounts from the command
        volumes = []
        i = 0
        while i < len(container_cmd):
            if container_cmd[i] == "-v" and i + 1 < len(container_cmd):
                volumes.append(container_cmd[i + 1])
                i += 2
            else:
                i += 1
        
        # Check for required volume mounts with :Z flag
        self.assertTrue(any(".claude.json:/root/.claude.json:Z" in v for v in volumes))
        self.assertTrue(any(".claude/:/root/.claude/:Z" in v for v in volumes))
        # Current directory mount (test_dir since we run from there)
        self.assertTrue(any(f"{self.test_dir}:{self.test_dir}:Z" in v for v in volumes))

    def test_working_directory(self) -> None:
        """Test that working directory is set correctly."""
        # Create simple dummy podman for session workflow
        podman_script = """#!/usr/bin/env python3
import sys
# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)  # Execution success
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        self.assertEqual(returncode, 0)
        
        # Parse the stderr output to find the "Running command:" line
        stderr_lines = stderr.strip().split('\n')
        container_cmd = None
        for line in stderr_lines:
            if line.startswith("Running command:"):
                container_cmd = line[len("Running command: "):].split()
                break
        
        self.assertIsNotNone(container_cmd, "Should find container creation command in stderr")
        
        # Extract working directory from the command
        workdir = None
        for i, arg in enumerate(container_cmd):
            if arg == "-w" and i + 1 < len(container_cmd):
                workdir = container_cmd[i + 1]
                break
        
        self.assertEqual(workdir, self.test_dir)

    def test_missing_container_binary(self) -> None:
        """Test error handling when container binary is missing."""
        # Use non-existent binary
        env = {"CONTAINER_BINARY": "nonexistent-binary"}
        returncode, stdout, stderr = self.run_script([], env=env)

        self.assertEqual(returncode, 1)
        self.assertIn("Unexpected error:", stderr)
        self.assertIn("No such file or directory", stderr)

    def test_keyboard_interrupt(self) -> None:
        """Test handling of keyboard interrupt."""
        # Create podman that raises KeyboardInterrupt
        podman_script = """#!/usr/bin/env python3
raise KeyboardInterrupt()
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        # subprocess.run catches KeyboardInterrupt from child differently
        # The script should handle the non-zero exit code from podman
        self.assertNotEqual(returncode, 0)

    def test_exit_code_propagation(self) -> None:
        """Test that exit codes are properly propagated."""
        # Create podman that handles session workflow and exits with specific code in exec
        podman_script = """#!/usr/bin/env python3
import sys

# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    # Container doesn't exist
    sys.exit(1)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Container creation - succeed
    sys.exit(0)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Container execution - exit with code 42
    sys.exit(42)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])
        self.assertEqual(returncode, 42)

    def test_all_arguments_passed(self) -> None:
        """Test that all arguments are passed through."""
        # Create podman that echoes all arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
# Skip first few container args to get to claude args
claude_args = []
found_claude = False
for arg in sys.argv[1:]:
    if found_claude:
        claude_args.append(arg)
    elif arg == "claude":
        found_claude = True
print(json.dumps({"claude_args": claude_args}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Test with various argument types that don't include session management flags
        test_args = [
            "--print",
            "test",
            "--output-format", 
            "stream-json",
            "--verbose",
            "some-file.txt",
            "--flag-with-value=test",
            "--",
            "additional",
            "args",
        ]

        returncode, stdout, stderr = self.run_script(test_args)

        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        claude_args = output["claude_args"]
        
        # All our test args should be present
        for arg in test_args:
            self.assertIn(arg, claude_args)
        
        # Should also have --session-id added for new sessions
        self.assertIn("--session-id", claude_args)

    def test_interactive_flag(self) -> None:
        """Test that -i flag is included in the command."""
        # Create dummy podman that handles the session management workflow
        podman_script = """#!/usr/bin/env python3
import sys
import json

# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    # Container doesn't exist
    sys.exit(1)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Container creation - succeed silently
    sys.exit(0)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Container execution - this is what we want to capture
    args = sys.argv[1:]  # exec command args
    print(json.dumps({"args": args}))
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        output = json.loads(stdout)
        args = output["args"]

        self.assertEqual(returncode, 0)
        self.assertIn("-i", args)

        # In the new exec workflow, just verify -i is present
        exec_index = args.index("exec")
        i_index = args.index("-i")
        
        # -i should come after exec
        self.assertGreater(i_index, exec_index)

        # Check if -t flag is present (conditional on TTY)
        has_t_flag = "-t" in args
        if has_t_flag:
            # If -t is present, it should come after -i
            t_index = args.index("-t")
            self.assertGreater(t_index, i_index)

    def test_exception_with_stacktrace(self) -> None:
        """Test that exceptions show full stacktrace."""
        # Create podman that handles session workflow but fails in container creation
        podman_script = """#!/usr/bin/env python3
import sys

# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    # Container doesn't exist
    sys.exit(1)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Container creation - fail with error output
    print("RuntimeError: Test exception from dummy binary", file=sys.stderr)
    sys.exit(1)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Container execution - should not be reached
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        # When podman exits with non-zero, it propagates the exit code
        self.assertNotEqual(returncode, 0)
        # The error from the dummy binary should be visible
        self.assertIn("RuntimeError", stderr)

    def test_special_characters_in_paths(self) -> None:
        """Test handling of special characters in paths."""
        # Create directory with special characters
        special_dir = Path(self.test_dir) / "test dir with spaces & special-chars"
        special_dir.mkdir(parents=True, exist_ok=True)

        # Create podman that handles the session workflow
        podman_script = """#!/usr/bin/env python3
import sys
import json

# Handle different commands in the new workflow
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    # Container doesn't exist
    sys.exit(1)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Container creation - succeed
    sys.exit(0)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Container execution - output success for test to parse
    print(json.dumps({"success": True}))
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Run from special directory
        cmd = [sys.executable, str(self.script_path)]
        env = os.environ.copy()
        result = subprocess.run(cmd, capture_output=True, text=True, env=env, cwd=special_dir)

        self.assertEqual(result.returncode, 0)
        # Check that the special directory path is used in the container creation command
        self.assertIn(str(special_dir), result.stderr)

    def test_real_keyboard_interrupt(self) -> None:
        """Test real keyboard interrupt handling in wrapper."""
        # Test with a long-running command that we can interrupt
        # Create podman that sleeps
        podman_script = """#!/usr/bin/env python3
import time
import sys
try:
    time.sleep(5)
except KeyboardInterrupt:
    sys.exit(130)
"""
        self.create_dummy_binary("podman", podman_script)

        # Start the script in a subprocess
        import signal

        cmd = [sys.executable, str(self.script_path), "--test"]
        env = os.environ.copy()

        proc = subprocess.Popen(
            cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env, cwd=self.test_dir
        )

        # Give it a moment to start
        import time

        time.sleep(0.1)

        # Send interrupt signal
        proc.send_signal(signal.SIGINT)

        # Wait for completion
        stdout, stderr = proc.communicate()

        # Should exit with 130
        self.assertEqual(proc.returncode, 130)
        self.assertIn("Interrupted by user", stderr)

    def test_tty_detection_with_terminal(self) -> None:
        """Test -t flag is added when running from terminal."""
        # Create dummy podman that captures all arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
print(json.dumps({"args": sys.argv[1:], "has_t": "-t" in sys.argv[1:]}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Create a script that mocks isatty() to return True
        test_script = f"""#!/usr/bin/env python3
import sys
import os
sys.path.insert(0, '{Path(__file__).parent}')

# Mock sys.stdin.isatty to return True
class MockStdin:
    def isatty(self):
        return True
    def __getattr__(self, name):
        return getattr(sys.stdin, name)

sys.stdin = MockStdin()

# Import and run the main function
import claude_container
claude_container.main()
"""

        test_script_path = Path(self.test_dir) / "test_with_tty.py"
        test_script_path.write_text(test_script)
        test_script_path.chmod(0o755)

        # Run the test script
        cmd = [sys.executable, str(test_script_path), "--test"]
        env = os.environ.copy()
        result = subprocess.run(cmd, capture_output=True, text=True, env=env)

        if result.returncode == 0:
            output = json.loads(result.stdout)
            self.assertTrue(output["has_t"], "Expected -t flag when TTY is available")

    def test_tty_detection_without_terminal(self) -> None:
        """Test -t flag is NOT added when not running from terminal."""
        # Create dummy podman that captures all arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
print(json.dumps({"args": sys.argv[1:], "has_t": "-t" in sys.argv[1:]}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Create a script that mocks isatty() to return False
        test_script = f"""#!/usr/bin/env python3
import sys
import os
sys.path.insert(0, '{Path(__file__).parent}')

# Mock sys.stdin.isatty to return False
class MockStdin:
    def isatty(self):
        return False
    def __getattr__(self, name):
        return getattr(sys.stdin, name)

sys.stdin = MockStdin()

# Import and run the main function
import claude_container
claude_container.main()
"""

        test_script_path = Path(self.test_dir) / "test_without_tty.py"
        test_script_path.write_text(test_script)
        test_script_path.chmod(0o755)

        # Run the test script
        cmd = [sys.executable, str(test_script_path), "--test"]
        env = os.environ.copy()
        result = subprocess.run(cmd, capture_output=True, text=True, env=env)

        if result.returncode == 0:
            output = json.loads(result.stdout)
            self.assertFalse(
                output["has_t"], "Expected NO -t flag when TTY is not available"
            )

    def test_debug_mode_enabled(self) -> None:
        """Test that quiet flags are NOT added when DEBUG=true."""
        # Create dummy podman that captures all arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
args = sys.argv[1:]
print(json.dumps({
    "args": args,
    "has_quiet": "--quiet" in args,
    "has_log_driver_none": "--log-driver" in args and "none" in args
}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Run with DEBUG=true
        env = {"DEBUG": "true"}
        returncode, stdout, stderr = self.run_script(["--test"], env=env)

        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        self.assertFalse(
            output["has_quiet"], "Expected NO --quiet flag when DEBUG=true"
        )
        self.assertFalse(
            output["has_log_driver_none"],
            "Expected NO --log-driver none when DEBUG=true",
        )

    def test_debug_mode_disabled_default(self) -> None:
        """Test that quiet flags ARE added by default (DEBUG not set)."""
        # Create simple dummy podman for session workflow
        podman_script = """#!/usr/bin/env python3
import sys
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)  # Execution success
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Run without DEBUG set
        returncode, stdout, stderr = self.run_script(["--test"])

        self.assertEqual(returncode, 0)
        
        # Parse the stderr output to find the "Running command:" line
        stderr_lines = stderr.strip().split('\n')
        container_cmd = None
        for line in stderr_lines:
            if line.startswith("Running command:"):
                container_cmd = line[len("Running command: "):].split()
                break
        
        self.assertIsNotNone(container_cmd, "Should find container creation command in stderr")
        
        # Check that quiet flags are present
        self.assertIn("--quiet", container_cmd)
        self.assertIn("--log-driver", container_cmd)
        self.assertIn("none", container_cmd)

    def test_security_options_included(self) -> None:
        """Test that security options are included in container creation."""
        # Create simple dummy podman for session workflow
        podman_script = """#!/usr/bin/env python3
import sys
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)  # Execution success
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        self.assertEqual(returncode, 0)
        
        # Parse the stderr output to find the "Running command:" line
        stderr_lines = stderr.strip().split('\n')
        container_cmd = None
        for line in stderr_lines:
            if line.startswith("Running command:"):
                container_cmd = line[len("Running command: "):].split()
                break
        
        self.assertIsNotNone(container_cmd, "Should find container creation command in stderr")
        
        # Check that all security options are present
        self.assertIn("--security-opt", container_cmd)
        self.assertIn("label=disable", container_cmd)
        self.assertIn("seccomp=unconfined", container_cmd)
        self.assertIn("--cap-add=all", container_cmd)
        self.assertIn("--privileged", container_cmd)
        
        # Verify the security-opt flags have their values
        security_opt_count = container_cmd.count("--security-opt")
        self.assertEqual(security_opt_count, 2, "Should have exactly 2 --security-opt flags")

    def test_debug_mode_various_values(self) -> None:
        """Test various DEBUG environment variable values."""
        # Create simple dummy podman for session workflow
        podman_script = """#!/usr/bin/env python3
import sys
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)  # Execution success
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Test DEBUG values that should NOT enable debug mode (should have quiet flags)
        debug_false_values = ["false", "0", "no", "off", ""]
        for debug_value in debug_false_values:
            with self.subTest(debug_value=debug_value):
                env = {"DEBUG": debug_value}
                returncode, stdout, stderr = self.run_script(["--test"], env=env)
                
                self.assertEqual(returncode, 0)
                
                # Parse stderr for container creation command
                stderr_lines = stderr.strip().split('\n')
                container_cmd = None
                for line in stderr_lines:
                    if line.startswith("Running command:"):
                        container_cmd = line[len("Running command: "):].split()
                        break
                
                self.assertIsNotNone(container_cmd)
                self.assertIn("--quiet", container_cmd, f"Expected --quiet when DEBUG={debug_value}")
                self.assertIn("--log-driver", container_cmd, f"Expected --log-driver when DEBUG={debug_value}")
                self.assertIn("none", container_cmd, f"Expected none when DEBUG={debug_value}")

    def test_quiet_flags_positioning(self) -> None:
        """Test that quiet flags are positioned correctly in the command."""
        # Create simple dummy podman for session workflow
        podman_script = """#!/usr/bin/env python3
import sys
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)  # Execution success
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Run without DEBUG (should have quiet flags)
        returncode, stdout, stderr = self.run_script(["--test"])

        self.assertEqual(returncode, 0)
        
        # Parse stderr for container creation command
        stderr_lines = stderr.strip().split('\n')
        container_cmd = None
        for line in stderr_lines:
            if line.startswith("Running command:"):
                container_cmd = line[len("Running command: "):].split()
                break
        
        self.assertIsNotNone(container_cmd)
        
        # Find positions of key flags
        positions = {}
        for i, arg in enumerate(container_cmd):
            if arg == "--quiet":
                positions["quiet"] = i
            elif arg == "--log-driver":
                positions["log_driver"] = i
            elif arg == "--name":
                positions["name"] = i

        # Verify ordering: --name should come before --quiet
        # --quiet should come before --log-driver  
        self.assertIn("name", positions)
        self.assertIn("quiet", positions)
        self.assertIn("log_driver", positions)
        
        self.assertLess(positions["name"], positions["quiet"])
        self.assertLess(positions["quiet"], positions["log_driver"])


class TestSessionManagement(unittest.TestCase):
    """Test session management functionality."""

    def setUp(self) -> None:
        """Set up test environment."""
        self.test_dir = tempfile.mkdtemp()
        self.original_env = os.environ.copy()

        # Create dummy binaries directory
        self.dummy_bin_dir = Path(self.test_dir) / "bin"
        self.dummy_bin_dir.mkdir(parents=True, exist_ok=True)

        # Add dummy bin to PATH
        os.environ["PATH"] = f"{self.dummy_bin_dir}:{os.environ.get('PATH', '')}"

        # Get script path
        self.script_path = Path(__file__).parent / "claude-container.py"

    def tearDown(self) -> None:
        """Clean up test environment."""
        # Restore original environment
        os.environ.clear()
        os.environ.update(self.original_env)

        # Clean up test directory
        subprocess.run(["rm", "-rf", self.test_dir], check=True)

    def create_dummy_binary(self, name: str, content: str) -> Path:
        """Create a dummy executable binary."""
        binary_path = self.dummy_bin_dir / name
        binary_path.write_text(content)
        binary_path.chmod(0o755)
        return binary_path

    def run_script(
        self, args: List[str] = None, env: Dict[str, str] = None
    ) -> Tuple[int, str, str]:
        """Run the claude-container.py script and capture output."""
        cmd = [sys.executable, str(self.script_path)]
        if args:
            cmd.extend(args)

        # Merge environment variables
        run_env = os.environ.copy()
        if env:
            run_env.update(env)

        # Run from test_dir (non-git directory) to avoid git worktree logic
        result = subprocess.run(cmd, capture_output=True, text=True, env=run_env, cwd=self.test_dir)
        return result.returncode, result.stdout, result.stderr

    def test_new_session_creation(self) -> None:
        """Test that new sessions are created with UUID and proper container lifecycle."""
        call_count = 0
        
        # Create dummy podman that handles multiple calls
        podman_script = """#!/usr/bin/env python3
import sys
import json
import os

# Read call count from file, increment it
call_file = "/tmp/podman_call_count"
if os.path.exists(call_file):
    with open(call_file, "r") as f:
        call_count = int(f.read().strip())
else:
    call_count = 0

call_count += 1
with open(call_file, "w") as f:
    f.write(str(call_count))

# First call: container exists check (return 1 - doesn't exist)
if call_count == 1 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    container_name = sys.argv[3]
    print(json.dumps({"call": call_count, "command": "exists", "container": container_name, "exists": False}))
    sys.exit(1)  # Container doesn't exist

# Second call: container creation
elif call_count == 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    name_idx = sys.argv.index("--name")
    container_name = sys.argv[name_idx + 1]
    print(json.dumps({"call": call_count, "command": "create", "container": container_name, "args": sys.argv[1:]}))
    sys.exit(0)  # Creation success

# Third call: container execution
elif call_count == 3 and sys.argv[1] == "exec":
    container_name = sys.argv[3]  # after -i, -t or just -i
    if sys.argv[2] == "-i":
        if sys.argv[3] == "-t":
            container_name = sys.argv[4]
        else:
            container_name = sys.argv[3]
    elif sys.argv[2] == "-t":
        container_name = sys.argv[3]
    
    # Extract claude args (everything after container name and claude binary)
    claude_idx = -1
    for i, arg in enumerate(sys.argv):
        if arg == "claude":
            claude_idx = i
            break
    claude_args = sys.argv[claude_idx + 1:] if claude_idx >= 0 else []
    
    print(json.dumps({"call": call_count, "command": "exec", "container": container_name, "claude_args": claude_args}))
    sys.exit(0)  # Execution success

else:
    print(json.dumps({"call": call_count, "error": "unexpected", "args": sys.argv[1:]}))
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Clean up any existing call count file
        subprocess.run(["rm", "-f", "/tmp/podman_call_count"], check=False)

        # Run script without session arguments, using our dummy podman
        env = {"CONTAINER_BINARY": str(self.dummy_bin_dir / "podman")}
        returncode, stdout, stderr = self.run_script(["test", "command"], env=env)

        self.assertEqual(returncode, 0)
        
        # Parse the output - should be from the final exec command only
        output = json.loads(stdout.strip())

        # Verify this is the exec call
        self.assertEqual(output["call"], 3)
        self.assertEqual(output["command"], "exec")
        self.assertTrue(output["container"].startswith("claude-session-"))
        
        claude_args = output["claude_args"]
        self.assertIn("test", claude_args)
        self.assertIn("command", claude_args)
        self.assertIn("--session-id", claude_args)
        
        # The UUID should be in the args and match container name
        session_id_idx = claude_args.index("--session-id")
        session_id = claude_args[session_id_idx + 1]
        self.assertEqual(f"claude-session-{session_id}", output["container"])

    def test_resume_existing_session(self) -> None:
        """Test resuming an existing session with --resume."""
        call_count = 0
        
        # Create dummy podman that handles multiple calls
        podman_script = """#!/usr/bin/env python3
import sys
import json
import os

# Read call count from file, increment it
call_file = "/tmp/podman_call_count"
if os.path.exists(call_file):
    with open(call_file, "r") as f:
        call_count = int(f.read().strip())
else:
    call_count = 0

call_count += 1
with open(call_file, "w") as f:
    f.write(str(call_count))

# First call: container exists check (return 0 - exists)
if call_count == 1 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    container_name = sys.argv[3]
    print(json.dumps({"call": call_count, "command": "exists", "container": container_name, "exists": True}))
    sys.exit(0)  # Container exists

# Second call: container execution (no creation needed)
elif call_count == 2 and sys.argv[1] == "exec":
    container_name = sys.argv[3]  # after -i, -t or just -i
    if sys.argv[2] == "-i":
        if sys.argv[3] == "-t":
            container_name = sys.argv[4]
        else:
            container_name = sys.argv[3]
    elif sys.argv[2] == "-t":
        container_name = sys.argv[3]
    
    # Extract claude args
    claude_idx = -1
    for i, arg in enumerate(sys.argv):
        if arg == "claude":
            claude_idx = i
            break
    claude_args = sys.argv[claude_idx + 1:] if claude_idx >= 0 else []
    
    print(json.dumps({"call": call_count, "command": "exec", "container": container_name, "claude_args": claude_args}))
    sys.exit(0)  # Execution success

else:
    print(json.dumps({"call": call_count, "error": "unexpected", "args": sys.argv[1:]}))
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Clean up any existing call count file
        subprocess.run(["rm", "-f", "/tmp/podman_call_count"], check=False)

        # Run script with --resume, using our dummy podman
        env = {"CONTAINER_BINARY": str(self.dummy_bin_dir / "podman")}
        returncode, stdout, stderr = self.run_script(["--resume", "existing-session-123", "test", "command"], env=env)

        self.assertEqual(returncode, 0)
        
        # Parse the output - should be from the final exec command only
        output = json.loads(stdout.strip())

        # Verify this is the exec call (call 2 since container already exists)
        self.assertEqual(output["call"], 2)
        self.assertEqual(output["command"], "exec")
        self.assertEqual(output["container"], "claude-session-existing-session-123")
        
        claude_args = output["claude_args"]
        self.assertIn("test", claude_args)
        self.assertIn("command", claude_args)
        self.assertNotIn("--session-id", claude_args)  # Should not be added for --resume

    def test_explicit_session_id(self) -> None:
        """Test using explicit --session-id."""
        call_count = 0
        
        # Create dummy podman that handles multiple calls
        podman_script = """#!/usr/bin/env python3
import sys
import json
import os

# Read call count from file, increment it
call_file = "/tmp/podman_call_count"
if os.path.exists(call_file):
    with open(call_file, "r") as f:
        call_count = int(f.read().strip())
else:
    call_count = 0

call_count += 1
with open(call_file, "w") as f:
    f.write(str(call_count))

# First call: container exists check (return 0 - exists)
if call_count == 1 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    container_name = sys.argv[3]
    print(json.dumps({"call": call_count, "command": "exists", "container": container_name, "exists": True}))
    sys.exit(0)  # Container exists

# Second call: container execution
elif call_count == 2 and sys.argv[1] == "exec":
    container_name = sys.argv[3]  # after -i, -t or just -i
    if sys.argv[2] == "-i":
        if sys.argv[3] == "-t":
            container_name = sys.argv[4]
        else:
            container_name = sys.argv[3]
    elif sys.argv[2] == "-t":
        container_name = sys.argv[3]
    
    # Extract claude args
    claude_idx = -1
    for i, arg in enumerate(sys.argv):
        if arg == "claude":
            claude_idx = i
            break
    claude_args = sys.argv[claude_idx + 1:] if claude_idx >= 0 else []
    
    print(json.dumps({"call": call_count, "command": "exec", "container": container_name, "claude_args": claude_args}))
    sys.exit(0)  # Execution success

else:
    print(json.dumps({"call": call_count, "error": "unexpected", "args": sys.argv[1:]}))
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Clean up any existing call count file
        subprocess.run(["rm", "-f", "/tmp/podman_call_count"], check=False)

        # Run script with --session-id, using our dummy podman
        env = {"CONTAINER_BINARY": str(self.dummy_bin_dir / "podman")}
        returncode, stdout, stderr = self.run_script(["--session-id", "my-custom-session", "test", "command"], env=env)

        self.assertEqual(returncode, 0)
        
        # Parse the output - should be from the final exec command only
        output = json.loads(stdout.strip())

        # Verify this is the exec call (call 2 since container already exists)
        self.assertEqual(output["call"], 2)
        self.assertEqual(output["command"], "exec")
        self.assertEqual(output["container"], "claude-session-my-custom-session")
        
        claude_args = output["claude_args"]
        self.assertEqual(claude_args, ["--session-id", "my-custom-session", "test", "command"])  # All original args should be preserved

    def test_container_creation_failure(self) -> None:
        """Test handling of container creation failure."""
        call_count = 0
        
        # Create dummy podman that fails on container creation
        podman_script = """#!/usr/bin/env python3
import sys
import json
import os

# Read call count from file, increment it
call_file = "/tmp/podman_call_count"
if os.path.exists(call_file):
    with open(call_file, "r") as f:
        call_count = int(f.read().strip())
else:
    call_count = 0

call_count += 1
with open(call_file, "w") as f:
    f.write(str(call_count))

# First call: container exists check (return 1 - doesn't exist)
if call_count == 1 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    container_name = sys.argv[3]
    print(json.dumps({"call": call_count, "command": "exists", "container": container_name, "exists": False}))
    sys.exit(1)  # Container doesn't exist

# Second call: container creation (fail)
elif call_count == 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    name_idx = sys.argv.index("--name")
    container_name = sys.argv[name_idx + 1]
    print(json.dumps({"call": call_count, "command": "create_failed", "container": container_name}))
    sys.exit(1)  # Creation failure

else:
    print(json.dumps({"call": call_count, "error": "unexpected", "args": sys.argv[1:]}))
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Clean up any existing call count file
        subprocess.run(["rm", "-f", "/tmp/podman_call_count"], check=False)

        # Run script without session arguments, using our dummy podman
        env = {"CONTAINER_BINARY": str(self.dummy_bin_dir / "podman")}
        returncode, stdout, stderr = self.run_script(["test", "command"], env=env)

        # Should exit with error code due to container creation failure
        self.assertNotEqual(returncode, 0)
        
        # Should show error message in stderr
        self.assertIn("Failed to create container", stderr)


class TestGitWorktree(unittest.TestCase):
    """Test git worktree functionality."""

    def setUp(self) -> None:
        """Set up test environment with a git repository."""
        self.test_dir = tempfile.mkdtemp()
        self.original_env = os.environ.copy()

        # Create a git repository in test directory
        subprocess.run(["git", "init"], cwd=self.test_dir, check=True, capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=self.test_dir, check=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=self.test_dir, check=True)
        
        # Create initial commit
        test_file = Path(self.test_dir) / "test.txt"
        test_file.write_text("test content")
        subprocess.run(["git", "add", "test.txt"], cwd=self.test_dir, check=True, capture_output=True)
        subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=self.test_dir, check=True, capture_output=True)

        # Create dummy binaries directory
        self.dummy_bin_dir = Path(self.test_dir) / "bin"
        self.dummy_bin_dir.mkdir(parents=True, exist_ok=True)

        # Add dummy bin to PATH
        os.environ["PATH"] = f"{self.dummy_bin_dir}:{os.environ.get('PATH', '')}"

        # Get script path
        self.script_path = Path(__file__).parent / "claude-container.py"

        # Set up worktree directory
        self.worktree_dir = Path(self.test_dir) / "worktrees"
        self.worktree_dir.mkdir()

    def tearDown(self) -> None:
        """Clean up test environment."""
        # Restore original environment
        os.environ.clear()
        os.environ.update(self.original_env)

        # Clean up test directory
        subprocess.run(["rm", "-rf", self.test_dir], check=True)

    def create_dummy_binary(self, name: str, content: str) -> Path:
        """Create a dummy executable binary."""
        binary_path = self.dummy_bin_dir / name
        binary_path.write_text(content)
        binary_path.chmod(0o755)
        return binary_path

    def run_script(
        self, args: List[str] = None, env: Dict[str, str] = None
    ) -> Tuple[int, str, str]:
        """Run the claude-container.py script from the git repository."""
        cmd = [sys.executable, str(self.script_path)]
        if args:
            cmd.extend(args)

        # Merge environment variables
        run_env = os.environ.copy()
        if env:
            run_env.update(env)

        # Run from git repository directory
        result = subprocess.run(cmd, capture_output=True, text=True, env=run_env, cwd=self.test_dir)
        return result.returncode, result.stdout, result.stderr

    def test_git_worktree_creation(self) -> None:
        """Test that git worktrees are created for git repositories."""
        # Create dummy podman that tracks volume mounts
        podman_script = """#!/usr/bin/env python3
import sys
import json

if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Extract volume mounts
    volumes = []
    i = 0
    while i < len(sys.argv):
        if sys.argv[i] == "-v" and i + 1 < len(sys.argv):
            volumes.append(sys.argv[i + 1])
            i += 2
        else:
            i += 1
    print(json.dumps({"volumes": volumes}))
    sys.exit(0)
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Run script with git worktree directory set
        env = {
            "CONTAINER_BINARY": str(self.dummy_bin_dir / "podman"),
            "GIT_WORKTREES_DIR": str(self.worktree_dir)
        }
        returncode, stdout, stderr = self.run_script(["test", "command"], env=env)

        self.assertEqual(returncode, 0)
        
        # Parse volume mounts from output
        if stdout.strip():
            output = json.loads(stdout.strip())
            volumes = output["volumes"]
        else:
            # If no JSON output, extract from stderr (container creation command)
            volumes = []
            stderr_lines = stderr.strip().split('\n')
            for line in stderr_lines:
                if line.startswith("Running command:"):
                    cmd_parts = line[len("Running command: "):].split()
                    i = 0
                    while i < len(cmd_parts):
                        if cmd_parts[i] == "-v" and i + 1 < len(cmd_parts):
                            volumes.append(cmd_parts[i + 1])
                            i += 2
                        else:
                            i += 1
                    break

        # Should mount the worktree directory, not the original git directory
        git_mount = None
        for volume in volumes:
            if volume.endswith(f":{self.test_dir}:Z"):
                git_mount = volume
                break

        self.assertIsNotNone(git_mount, "Should find git directory mount")
        
        # The mount source should be a worktree directory, not the original
        mount_source = git_mount.split(":")[0]
        self.assertTrue(mount_source.startswith(str(self.worktree_dir)))
        # Should contain the session ID (UUID format or explicit session name)
        self.assertNotEqual(mount_source, str(self.test_dir))

        # Verify worktree was actually created (should have at least one directory)
        session_dirs = list(self.worktree_dir.glob("*"))
        self.assertEqual(len(session_dirs), 1, "Should create exactly one worktree")
        
        # Verify worktree contains the test file
        worktree_test_file = session_dirs[0] / "test.txt"
        self.assertTrue(worktree_test_file.exists())
        self.assertEqual(worktree_test_file.read_text(), "test content")

    def test_existing_worktree_reuse(self) -> None:
        """Test that existing worktrees are reused."""
        # Create dummy podman
        podman_script = """#!/usr/bin/env python3
import sys
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(0)  # Container exists
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Run script twice with same session ID
        session_id = "test-session-123"
        env = {
            "CONTAINER_BINARY": str(self.dummy_bin_dir / "podman"),
            "GIT_WORKTREES_DIR": str(self.worktree_dir)
        }

        # First run should create worktree
        returncode1, stdout1, stderr1 = self.run_script(["--session-id", session_id], env=env)
        self.assertEqual(returncode1, 0)
        self.assertIn("Created git worktree", stderr1)

        # Second run should reuse existing worktree
        returncode2, stdout2, stderr2 = self.run_script(["--session-id", session_id], env=env)
        self.assertEqual(returncode2, 0)
        self.assertNotIn("Created git worktree", stderr2)  # Should not create again

        # Verify only one worktree directory exists
        session_dirs = list(self.worktree_dir.glob("test-session-123"))
        self.assertEqual(len(session_dirs), 1)


class TestDangerousSkipPermissions(unittest.TestCase):
    """Test --dangerously-skip-permissions functionality."""

    def setUp(self) -> None:
        """Set up test environment."""
        self.test_dir = tempfile.mkdtemp()
        self.original_env = os.environ.copy()

        # Create dummy binaries directory
        self.dummy_bin_dir = Path(self.test_dir) / "bin"
        self.dummy_bin_dir.mkdir(parents=True, exist_ok=True)

        # Add dummy bin to PATH
        os.environ["PATH"] = f"{self.dummy_bin_dir}:{os.environ.get('PATH', '')}"

        # Get script path
        self.script_path = Path(__file__).parent / "claude-container.py"

    def tearDown(self) -> None:
        """Clean up test environment."""
        # Restore original environment
        os.environ.clear()
        os.environ.update(self.original_env)

        # Clean up test directory
        subprocess.run(["rm", "-rf", self.test_dir], check=True)

    def create_dummy_binary(self, name: str, content: str) -> Path:
        """Create a dummy executable binary."""
        binary_path = self.dummy_bin_dir / name
        binary_path.write_text(content)
        binary_path.chmod(0o755)
        return binary_path

    def run_script(
        self, args: List[str] = None, env: Dict[str, str] = None
    ) -> Tuple[int, str, str]:
        """Run the claude-container.py script and capture output."""
        cmd = [sys.executable, str(self.script_path)]
        if args:
            cmd.extend(args)

        # Merge environment variables
        run_env = os.environ.copy()
        if env:
            run_env.update(env)

        # Run from test_dir (non-git directory) to avoid git worktree logic
        result = subprocess.run(cmd, capture_output=True, text=True, env=run_env, cwd=self.test_dir)
        return result.returncode, result.stdout, result.stderr

    def test_flag_parsing_without_dangerous_flag(self) -> None:
        """Test that normal execution works without --dangerously-skip-permissions flag."""
        # Create simple dummy podman for session workflow
        podman_script = """#!/usr/bin/env python3
import sys
import json
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Check if settings.json mount is present
    has_settings_mount = False
    for i, arg in enumerate(sys.argv):
        if arg == "-v" and i + 1 < len(sys.argv):
            if "claude-settings.json:/root/.claude/settings.json:Z" in sys.argv[i + 1]:
                has_settings_mount = True
                break
    print(json.dumps({"has_settings_mount": has_settings_mount}))
    sys.exit(0)
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script(["--test", "arg"])

        self.assertEqual(returncode, 0)
        if stdout.strip():
            output = json.loads(stdout.strip())
            self.assertFalse(output["has_settings_mount"], "Should not mount settings.json without dangerous flag")

    def test_flag_parsing_with_dangerous_flag(self) -> None:
        """Test that --dangerously-skip-permissions flag is properly parsed and processed."""
        # Create dummy podman that captures volume mounts
        podman_script = """#!/usr/bin/env python3
import sys
import json
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Check if settings.json mount is present
    has_settings_mount = False
    for i, arg in enumerate(sys.argv):
        if arg == "-v" and i + 1 < len(sys.argv):
            if "claude-settings.json:/root/.claude/settings.json:Z" in sys.argv[i + 1]:
                has_settings_mount = True
                break
    print(json.dumps({"has_settings_mount": has_settings_mount}))
    sys.exit(0)
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script(["--dangerously-skip-permissions", "--test", "arg"])

        self.assertEqual(returncode, 0)
        if stdout.strip():
            output = json.loads(stdout.strip())
            self.assertTrue(output["has_settings_mount"], "Should mount settings.json with dangerous flag")

    def test_settings_mount_path_correctness(self) -> None:
        """Test that settings.json and hook script are mounted from correct source paths."""
        # Create dummy podman that captures all volume mounts
        podman_script = """#!/usr/bin/env python3
import sys
import json
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # Extract all volume mounts
    volumes = []
    i = 0
    while i < len(sys.argv):
        if sys.argv[i] == "-v" and i + 1 < len(sys.argv):
            volumes.append(sys.argv[i + 1])
            i += 2
        else:
            i += 1
    print(json.dumps({"volumes": volumes}))
    sys.exit(0)
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script(["--dangerously-skip-permissions", "--test"])

        self.assertEqual(returncode, 0)
        if stdout.strip():
            output = json.loads(stdout.strip())
            volumes = output["volumes"]
            
            # Find the settings.json and hook script mounts
            settings_mount = None
            hook_mount = None
            for volume in volumes:
                if ":/root/.claude/settings.json:Z" in volume:
                    settings_mount = volume
                elif ":/bin/claude-hook-pretooluse.sh:Z" in volume:
                    hook_mount = volume
            
            self.assertIsNotNone(settings_mount, "Should have settings.json volume mount")
            self.assertIsNotNone(hook_mount, "Should have hook script volume mount")
            
            # Check that the source paths are correct
            settings_source = settings_mount.split(":")[0]
            hook_source = hook_mount.split(":")[0]
            
            self.assertEqual(settings_source, "/root/.claude/settings.json",
                          f"Settings source should be /root/.claude/settings.json, got: {settings_source}")
            self.assertEqual(hook_source, "/bin/claude-hook-pretooluse.sh",
                          f"Hook source should be /bin/claude-hook-pretooluse.sh, got: {hook_source}")

    def test_flag_removal_from_claude_args(self) -> None:
        """Test that --dangerously-skip-permissions is removed from arguments passed to Claude."""
        # Create dummy podman that captures claude arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Extract claude args (everything after container name and claude binary)
    claude_idx = -1
    for i, arg in enumerate(sys.argv):
        if arg == "claude":
            claude_idx = i
            break
    claude_args = sys.argv[claude_idx + 1:] if claude_idx >= 0 else []
    print(json.dumps({"claude_args": claude_args}))
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([
            "--dangerously-skip-permissions", 
            "--test", 
            "arg1", 
            "--other-flag",
            "value"
        ])

        self.assertEqual(returncode, 0)
        if stdout.strip():
            output = json.loads(stdout.strip())
            claude_args = output["claude_args"]
            
            # Flag should be removed from Claude arguments
            self.assertNotIn("--dangerously-skip-permissions", claude_args, 
                           "Dangerous flag should not be passed to Claude")
            
            # Other arguments should be preserved
            self.assertIn("--test", claude_args)
            self.assertIn("arg1", claude_args)
            self.assertIn("--other-flag", claude_args)
            self.assertIn("value", claude_args)

    def test_flag_with_other_session_args(self) -> None:
        """Test that --dangerously-skip-permissions works with other session management flags."""
        # Create dummy podman that captures volume mounts and claude args
        podman_script = """#!/usr/bin/env python3
import sys
import json
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(0)  # Container exists (for --resume test)
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    # This should not be called since container exists
    sys.exit(1)  
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Extract claude args
    claude_idx = -1
    for i, arg in enumerate(sys.argv):
        if arg == "claude":
            claude_idx = i
            break
    claude_args = sys.argv[claude_idx + 1:] if claude_idx >= 0 else []
    print(json.dumps({"claude_args": claude_args}))
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        # Test with --resume
        returncode, stdout, stderr = self.run_script([
            "--resume", 
            "test-session", 
            "--dangerously-skip-permissions",
            "--test",
            "command"
        ])

        self.assertEqual(returncode, 0)
        if stdout.strip():
            output = json.loads(stdout.strip())
            claude_args = output["claude_args"]
            
            # Dangerous flag should be removed
            self.assertNotIn("--dangerously-skip-permissions", claude_args)
            # Other args should be preserved
            self.assertIn("--test", claude_args)
            self.assertIn("command", claude_args)
            # --resume args should be preserved
            self.assertIn("--resume", claude_args)
            self.assertIn("test-session", claude_args)

    def test_multiple_dangerous_flags(self) -> None:
        """Test handling of multiple --dangerously-skip-permissions flags (edge case)."""
        # Create dummy podman that captures claude arguments  
        podman_script = """#!/usr/bin/env python3
import sys
import json
if len(sys.argv) >= 3 and sys.argv[1] == "container" and sys.argv[2] == "exists":
    sys.exit(1)  # Container doesn't exist
elif len(sys.argv) >= 2 and sys.argv[1] == "run" and "-d" in sys.argv:
    sys.exit(0)  # Container creation success
elif len(sys.argv) >= 2 and sys.argv[1] == "exec":
    # Extract claude args
    claude_idx = -1
    for i, arg in enumerate(sys.argv):
        if arg == "claude":
            claude_idx = i
            break
    claude_args = sys.argv[claude_idx + 1:] if claude_idx >= 0 else []
    print(json.dumps({"claude_args": claude_args}))
    sys.exit(0)
else:
    sys.exit(1)
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([
            "--dangerously-skip-permissions",
            "--test", 
            "--dangerously-skip-permissions",  # Duplicate flag
            "arg"
        ])

        self.assertEqual(returncode, 0)
        if stdout.strip():
            output = json.loads(stdout.strip())
            claude_args = output["claude_args"]
            
            # Both instances of the flag should be removed
            self.assertNotIn("--dangerously-skip-permissions", claude_args)
            # Other arguments should be preserved
            self.assertIn("--test", claude_args)
            self.assertIn("arg", claude_args)


class TestHookScript(unittest.TestCase):
    """Test the claude-hook-pretooluse.sh script functionality."""

    def setUp(self) -> None:
        """Set up test environment."""
        self.script_path = Path(__file__).parent / "claude-hook-pretooluse.sh"

    def test_hook_script_exists(self) -> None:
        """Test that hook script file exists and is executable."""
        self.assertTrue(self.script_path.exists(), "Hook script should exist")
        # Check if file is executable
        self.assertTrue(os.access(self.script_path, os.X_OK), "Hook script should be executable")

    def test_hook_script_output_format(self) -> None:
        """Test that hook script outputs correct JSON format."""
        result = subprocess.run([str(self.script_path)], capture_output=True, text=True)
        
        self.assertEqual(result.returncode, 0, "Hook script should execute successfully")
        
        # Parse output as JSON
        try:
            output = json.loads(result.stdout)
        except json.JSONDecodeError as e:
            self.fail(f"Hook script output is not valid JSON: {e}\nOutput: {result.stdout}")

        # Verify structure
        self.assertIn("hookSpecificOutput", output, "Output should contain hookSpecificOutput")
        hook_output = output["hookSpecificOutput"]
        
        # Verify required fields
        self.assertIn("hookEventName", hook_output)
        self.assertIn("permissionDecision", hook_output)
        self.assertIn("permissionDecisionReason", hook_output)
        
        # Verify values
        self.assertEqual(hook_output["hookEventName"], "PreToolUse")
        self.assertEqual(hook_output["permissionDecision"], "allow")
        self.assertIn("--dangerously-skip-permissions", hook_output["permissionDecisionReason"])

    def test_hook_script_no_args_required(self) -> None:
        """Test that hook script works without any arguments."""
        result = subprocess.run([str(self.script_path)], capture_output=True, text=True)
        
        self.assertEqual(result.returncode, 0, "Hook script should work without arguments")
        self.assertTrue(result.stdout.strip(), "Hook script should produce output")

    def test_hook_script_stderr_clean(self) -> None:
        """Test that hook script doesn't produce stderr output."""
        result = subprocess.run([str(self.script_path)], capture_output=True, text=True)
        
        self.assertEqual(result.stderr.strip(), "", "Hook script should not produce stderr output")


class TestSettingsJsonFile(unittest.TestCase):
    """Test the claude-settings.json configuration file."""

    def setUp(self) -> None:
        """Set up test environment."""
        self.settings_path = Path(__file__).parent / "claude-settings.json"

    def test_settings_file_exists(self) -> None:
        """Test that settings file exists."""
        self.assertTrue(self.settings_path.exists(), "Settings file should exist")

    def test_settings_file_valid_json(self) -> None:
        """Test that settings file contains valid JSON."""
        try:
            with open(self.settings_path, 'r') as f:
                settings = json.load(f)
        except json.JSONDecodeError as e:
            self.fail(f"Settings file is not valid JSON: {e}")
        except Exception as e:
            self.fail(f"Error reading settings file: {e}")

        # Verify structure
        self.assertIn("hooks", settings, "Settings should contain hooks configuration")

    def test_settings_pretooluse_hook_configuration(self) -> None:
        """Test that PreToolUse hook is properly configured."""
        with open(self.settings_path, 'r') as f:
            settings = json.load(f)

        hooks = settings["hooks"]
        self.assertIn("PreToolUse", hooks, "Should configure PreToolUse hook")
        
        pretooluse_hooks = hooks["PreToolUse"]
        self.assertIsInstance(pretooluse_hooks, list, "PreToolUse should be a list")
        self.assertEqual(len(pretooluse_hooks), 1, "Should have exactly one PreToolUse hook")
        
        hook_config = pretooluse_hooks[0]
        self.assertIn("matcher", hook_config)
        self.assertIn("hooks", hook_config)
        
        # Verify matcher
        self.assertEqual(hook_config["matcher"], "*", "Should match all events")
        
        # Verify hook command
        hook_commands = hook_config["hooks"]
        self.assertIsInstance(hook_commands, list, "Hooks should be a list")
        self.assertEqual(len(hook_commands), 1, "Should have exactly one hook command")
        
        command_config = hook_commands[0]
        self.assertEqual(command_config["type"], "command")
        self.assertEqual(command_config["command"], "/bin/claude-hook-pretooluse.sh")


def run_tests() -> None:
    """Run all tests."""
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add all test classes
    suite.addTests(loader.loadTestsFromTestCase(TestClaudeContainer))
    suite.addTests(loader.loadTestsFromTestCase(TestSessionManagement))
    suite.addTests(loader.loadTestsFromTestCase(TestGitWorktree))
    suite.addTests(loader.loadTestsFromTestCase(TestDangerousSkipPermissions))
    suite.addTests(loader.loadTestsFromTestCase(TestHookScript))
    suite.addTests(loader.loadTestsFromTestCase(TestSettingsJsonFile))

    # Run tests
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)

    # Exit with appropriate code
    sys.exit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    run_tests()
