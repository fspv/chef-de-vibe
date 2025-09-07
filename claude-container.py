#!/usr/bin/env python3
"""
Claude container wrapper script.
Runs claude commands inside a podman container with proper volume mounts.
"""

import os
import subprocess
import sys
import traceback
from pathlib import Path


def main():
    # Get the base image from environment variable or use default
    base_image = os.environ.get(
        "CLAUDE_CONTAINER_IMAGE", "nuhotetotniksvoboden/claudecodeui:latest"
    )

    # Get claude binary path from environment variable or use default
    claude_binary = os.environ.get("CLAUDE_BINARY", "claude")

    # Get container management binary from environment variable or use default
    container_binary = os.environ.get("CONTAINER_BINARY", "podman")

    # Check if debug mode is enabled
    debug_mode = os.environ.get("DEBUG", "").lower() in ("true", "1", "yes")

    # Get HOME directory
    home_dir = os.path.expanduser("~")

    # Get current working directory
    current_dir = os.getcwd()

    # Build the container command
    container_cmd = [
        container_binary,
        "run",
        "--rm",  # Remove container after execution
    ]

    # Add quiet flags when not in debug mode
    if not debug_mode:
        container_cmd.extend(
            [
                "--quiet",  # Suppress output information when pulling images
                "--log-driver",
                "none",  # Disable container logging
            ]
        )

    container_cmd.append("-i")  # Keep STDIN open for interactive use

    # Add -t flag only if running from a terminal
    if sys.stdin.isatty():
        container_cmd.append("-t")

    container_cmd.extend(
        [
            "-v",
            f"{home_dir}/.claude.json:/root/.claude.json:Z",
            "-v",
            f"{home_dir}/.claude/:/root/.claude/:Z",
            "-v",
            f"{current_dir}:{current_dir}:Z",
            "-w",
            current_dir,  # Set working directory inside container
            base_image,
            claude_binary,
        ]
    )

    # Add all command line arguments passed to this script
    container_cmd.extend(sys.argv[1:])

    # Print the full command to stderr for debugging
    print(f"Running command: {' '.join(container_cmd)}", file=sys.stderr)

    try:
        # Execute the container command
        result = subprocess.run(container_cmd, check=False)
        sys.exit(result.returncode)
    except KeyboardInterrupt:
        print("\nInterrupted by user", file=sys.stderr)
        traceback.print_exc()
        sys.exit(130)
    except Exception:
        print("Error occurred while running container command:", file=sys.stderr)
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
