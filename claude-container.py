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
    base_image = os.environ.get("CLAUDE_CONTAINER_IMAGE", "nuhotetotniksvoboden/claudecodeui:latest")
    
    # Get claude binary path from environment variable or use default
    claude_binary = os.environ.get("CLAUDE_BINARY", "claude")
    
    # Get container management binary from environment variable or use default
    container_binary = os.environ.get("CONTAINER_BINARY", "podman")
    
    # Get HOME directory
    home_dir = os.path.expanduser("~")
    
    # Get current working directory
    current_dir = os.getcwd()
    
    # Build the container command
    container_cmd = [
        container_binary, "run",
        "--rm",  # Remove container after execution
        "-v", f"{home_dir}/.claude.json:/root/.claude.json",
        "-v", f"{home_dir}/.claude/:/root/.claude/",
        "-v", f"{current_dir}:{current_dir}",
        "-w", current_dir,  # Set working directory inside container
        "--restart", "unless-stopped",
        base_image,
        claude_binary
    ]
    
    # Add all command line arguments passed to this script
    container_cmd.extend(sys.argv[1:])
    
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