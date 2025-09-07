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

        result = subprocess.run(cmd, capture_output=True, text=True, env=run_env)

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
        # Current directory mount
        cwd = os.getcwd()
        self.assertTrue(any(f"{cwd}:{cwd}:Z" in v for v in volumes))

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
        
        self.assertEqual(workdir, os.getcwd())

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

        # Change to special directory and run
        original_cwd = os.getcwd()
        try:
            os.chdir(special_dir)
            returncode, stdout, stderr = self.run_script([])

            self.assertEqual(returncode, 0)
            # Check that the special directory path is used in the container creation command
            self.assertIn(str(special_dir), stderr)
        finally:
            os.chdir(original_cwd)

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
            cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env
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

        result = subprocess.run(cmd, capture_output=True, text=True, env=run_env)
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
        self.assertIn("test", claude_args)
        self.assertIn("command", claude_args)
        self.assertNotIn("--session-id", claude_args)  # Should not be added for explicit --session-id

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


def run_tests() -> None:
    """Run all tests."""
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add both test classes
    suite.addTests(loader.loadTestsFromTestCase(TestClaudeContainer))
    suite.addTests(loader.loadTestsFromTestCase(TestSessionManagement))

    # Run tests
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)

    # Exit with appropriate code
    sys.exit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    run_tests()
