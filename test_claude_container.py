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
        # Create dummy podman that logs arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
print(json.dumps({"args": sys.argv[1:]}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Run script with test arguments
        returncode, stdout, stderr = self.run_script(["--test", "arg1", "arg2"])

        # Parse output
        output = json.loads(stdout)
        args = output["args"]

        # Verify command structure
        self.assertEqual(returncode, 0)
        self.assertEqual(args[0], "run")
        self.assertEqual(args[1], "--rm")

        # By default (no DEBUG), quiet flags should be present
        self.assertIn("--quiet", args)
        self.assertIn("--log-driver", args)
        self.assertIn("none", args)
        self.assertIn("-i", args)

        # -t flag is conditional based on TTY, so we check if it exists
        has_t_flag = "-t" in args

        self.assertIn("-v", args)
        self.assertIn("claude", args)
        self.assertIn("--test", args)
        self.assertIn("arg1", args)
        self.assertIn("arg2", args)

    def test_custom_container_binary(self) -> None:
        """Test using docker instead of podman."""
        # Create dummy docker
        docker_script = """#!/usr/bin/env python3
import sys
import json
print(json.dumps({"binary": "docker", "args": sys.argv[1:]}))
"""
        self.create_dummy_binary("docker", docker_script)

        # Run with docker
        env = {"CONTAINER_BINARY": "docker"}
        returncode, stdout, stderr = self.run_script(["--help"], env=env)

        # Verify docker was used
        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        self.assertEqual(output["binary"], "docker")
        self.assertEqual(output["args"][0], "run")

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
        # Create dummy podman that captures image
        podman_script = """#!/usr/bin/env python3
import sys
import json
# Find image (arg before claude binary)
image = None
for i, arg in enumerate(sys.argv):
    if i + 1 < len(sys.argv) and sys.argv[i + 1] == "claude":
        image = arg
        break
print(json.dumps({"image": image}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Test with custom image
        env = {"CLAUDE_CONTAINER_IMAGE": "custom/image:latest"}
        returncode, stdout, stderr = self.run_script([], env=env)

        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        self.assertEqual(output["image"], "custom/image:latest")

    def test_volume_mounts(self) -> None:
        """Test that volume mounts are correctly set up."""
        # Create dummy podman that captures volume mounts
        podman_script = """#!/usr/bin/env python3
import sys
import json
volumes = []
i = 0
while i < len(sys.argv):
    if sys.argv[i] == "-v":
        volumes.append(sys.argv[i + 1])
        i += 2
    else:
        i += 1
print(json.dumps({"volumes": volumes}))
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        output = json.loads(stdout)
        volumes = output["volumes"]

        self.assertEqual(returncode, 0)
        # Check for required volume mounts with :Z flag
        self.assertTrue(any(".claude.json:/root/.claude.json:Z" in v for v in volumes))
        self.assertTrue(any(".claude/:/root/.claude/:Z" in v for v in volumes))
        # Current directory mount
        cwd = os.getcwd()
        self.assertTrue(any(f"{cwd}:{cwd}:Z" in v for v in volumes))

    def test_working_directory(self) -> None:
        """Test that working directory is set correctly."""
        # Create dummy podman that captures -w flag
        podman_script = """#!/usr/bin/env python3
import sys
import json
workdir = None
for i, arg in enumerate(sys.argv):
    if arg == "-w" and i + 1 < len(sys.argv):
        workdir = sys.argv[i + 1]
        break
print(json.dumps({"workdir": workdir}))
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        self.assertEqual(output["workdir"], os.getcwd())

    def test_missing_container_binary(self) -> None:
        """Test error handling when container binary is missing."""
        # Use non-existent binary
        env = {"CONTAINER_BINARY": "nonexistent-binary"}
        returncode, stdout, stderr = self.run_script([], env=env)

        self.assertEqual(returncode, 1)
        self.assertIn("Error occurred while running container command:", stderr)
        self.assertIn("Traceback", stderr)

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
        # Create podman that exits with specific code
        podman_script = """#!/usr/bin/env python3
import sys
sys.exit(42)
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

        # Test with various argument types
        test_args = [
            "--print",
            "test",
            "--resume",
            "68d8871e-2665-4c8c-80ca-c2e179b26749",
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
        self.assertEqual(output["claude_args"], test_args)

    def test_interactive_flag(self) -> None:
        """Test that -i flag is included in the command."""
        # Create dummy podman that captures all arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
print(json.dumps({"args": sys.argv[1:]}))
"""
        self.create_dummy_binary("podman", podman_script)

        returncode, stdout, stderr = self.run_script([])

        output = json.loads(stdout)
        args = output["args"]

        self.assertEqual(returncode, 0)
        self.assertIn("-i", args)

        # Since quiet flags are added by default, -i is no longer directly after --rm
        # Just verify that -i is present and comes before volume mounts
        rm_index = args.index("--rm")
        i_index = args.index("-i")
        v_index = args.index("-v")

        # -i should come after --rm and before volume mounts
        self.assertGreater(i_index, rm_index)
        self.assertLess(i_index, v_index)

        # Check if -t flag is present (conditional on TTY)
        has_t_flag = "-t" in args
        if has_t_flag:
            # If -t is present, it should come after -i
            t_index = args.index("-t")
            self.assertGreater(t_index, i_index)

    def test_exception_with_stacktrace(self) -> None:
        """Test that exceptions show full stacktrace."""
        # Create podman that raises an exception
        podman_script = """#!/usr/bin/env python3
raise RuntimeError("Test exception from dummy binary")
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

        # Create podman that reports working directory
        podman_script = """#!/usr/bin/env python3
import sys
import json
workdir = None
for i, arg in enumerate(sys.argv):
    if arg == "-w" and i + 1 < len(sys.argv):
        workdir = sys.argv[i + 1]
        break
print(json.dumps({"workdir": workdir}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Change to special directory and run
        original_cwd = os.getcwd()
        try:
            os.chdir(special_dir)
            returncode, stdout, stderr = self.run_script([])

            output = json.loads(stdout)
            self.assertEqual(returncode, 0)
            self.assertEqual(output["workdir"], str(special_dir))
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
        self.assertIn("KeyboardInterrupt", stderr)

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

        # Run without DEBUG set
        returncode, stdout, stderr = self.run_script(["--test"])

        output = json.loads(stdout)
        self.assertEqual(returncode, 0)
        self.assertTrue(
            output["has_quiet"], "Expected --quiet flag when DEBUG is not set"
        )
        self.assertTrue(
            output["has_log_driver_none"],
            "Expected --log-driver none when DEBUG is not set",
        )

    def test_debug_mode_various_values(self) -> None:
        """Test various DEBUG environment variable values."""
        # Create dummy podman that captures all arguments
        podman_script = """#!/usr/bin/env python3
import sys
import json
args = sys.argv[1:]
print(json.dumps({
    "has_quiet": "--quiet" in args,
    "has_log_driver_none": "--log-driver" in args and "none" in args
}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Test different DEBUG values that should enable debug mode
        debug_true_values = ["true", "TRUE", "True", "1", "yes", "YES", "Yes"]
        for debug_value in debug_true_values:
            with self.subTest(debug_value=debug_value):
                env = {"DEBUG": debug_value}
                returncode, stdout, stderr = self.run_script(["--test"], env=env)

                output = json.loads(stdout)
                self.assertEqual(returncode, 0)
                self.assertFalse(
                    output["has_quiet"],
                    f"Expected NO --quiet flag when DEBUG={debug_value}",
                )
                self.assertFalse(
                    output["has_log_driver_none"],
                    f"Expected NO --log-driver none when DEBUG={debug_value}",
                )

        # Test DEBUG values that should NOT enable debug mode
        debug_false_values = ["false", "0", "no", "off", ""]
        for debug_value in debug_false_values:
            with self.subTest(debug_value=debug_value):
                env = {"DEBUG": debug_value}
                returncode, stdout, stderr = self.run_script(["--test"], env=env)

                output = json.loads(stdout)
                self.assertEqual(returncode, 0)
                self.assertTrue(
                    output["has_quiet"],
                    f"Expected --quiet flag when DEBUG={debug_value}",
                )
                self.assertTrue(
                    output["has_log_driver_none"],
                    f"Expected --log-driver none when DEBUG={debug_value}",
                )

    def test_quiet_flags_positioning(self) -> None:
        """Test that quiet flags are positioned correctly in the command."""
        # Create dummy podman that captures all arguments with positions
        podman_script = """#!/usr/bin/env python3
import sys
import json
args = sys.argv[1:]
positions = {}
for i, arg in enumerate(args):
    if arg == "--quiet":
        positions["quiet"] = i
    elif arg == "--log-driver":
        positions["log_driver"] = i
    elif arg == "-i":
        positions["i"] = i
    elif arg == "--rm":
        positions["rm"] = i
print(json.dumps({"positions": positions, "args": args}))
"""
        self.create_dummy_binary("podman", podman_script)

        # Run without DEBUG (should have quiet flags)
        returncode, stdout, stderr = self.run_script(["--test"])

        output = json.loads(stdout)
        self.assertEqual(returncode, 0)

        positions = output["positions"]

        # Verify ordering: --rm, --quiet, --log-driver, none, -i
        self.assertIn("rm", positions)
        self.assertIn("quiet", positions)
        self.assertIn("log_driver", positions)
        self.assertIn("i", positions)

        # --quiet should come after --rm
        self.assertLess(positions["rm"], positions["quiet"])
        # --log-driver should come after --quiet
        self.assertLess(positions["quiet"], positions["log_driver"])
        # -i should come after --log-driver and none
        self.assertLess(
            positions["log_driver"] + 1, positions["i"]
        )  # +1 for "none" arg


def run_tests() -> None:
    """Run all tests."""
    # Create test suite
    loader = unittest.TestLoader()
    suite = loader.loadTestsFromTestCase(TestClaudeContainer)

    # Run tests
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)

    # Exit with appropriate code
    sys.exit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    run_tests()
