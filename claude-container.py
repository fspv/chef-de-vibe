#!/usr/bin/env python3
"""
Claude container wrapper script.
Runs claude commands inside a podman container with proper volume mounts.
"""

import argparse
import os
import subprocess
import sys
import traceback
import uuid
from pathlib import Path


def parse_args():
    """Parse command line arguments to extract session-related flags."""
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--resume", type=str, help="Resume session with session ID")
    parser.add_argument("--session-id", type=str, help="Session ID")
    
    # Parse known args to avoid errors with other claude arguments
    known_args, remaining_args = parser.parse_known_args()
    return known_args, remaining_args


def get_session_info(args):
    """Get session ID and determine if we need to add --session-id to claude args."""
    if args.resume:
        # Use session ID from --resume argument, session args already present
        return args.resume, False
    elif args.session_id:
        # Use provided session ID, session args already present
        return args.session_id, False
    else:
        # Generate new UUID session ID and add it to claude args
        return str(uuid.uuid4()), True


def container_exists(container_binary: str, container_name: str) -> bool:
    """Check if a container with the given name exists."""
    cmd = [container_binary, "container", "exists", container_name]
    
    print(f"Checking if container exists: {' '.join(cmd)}", file=sys.stderr)
    
    result = subprocess.run(cmd, capture_output=True, check=False)
    return result.returncode == 0


def create_container(container_binary: str, container_name: str, base_image: str, 
                    home_dir: str, current_dir: str, debug_mode: bool):
    """Create a new container with sleep infinity command."""
    container_cmd = [
        container_binary,
        "run",
        "-d",  # Run in detached mode
        "--name", container_name,
    ]

    # Add quiet flags when not in debug mode
    if not debug_mode:
        container_cmd.extend([
            "--quiet",  # Suppress output information when pulling images
            "--log-driver", "none",  # Disable container logging
        ])

    container_cmd.extend([
        "-v", f"{home_dir}/.claude.json:/root/.claude.json:Z",
        "-v", f"{home_dir}/.claude/:/root/.claude/:Z",
        "-v", f"{current_dir}:{current_dir}:Z",
        "-w", current_dir,  # Set working directory inside container
        base_image,
        "sleep", "infinity"
    ])

    # Print the container creation command to stdout for testing
    print(f"Running command: {' '.join(container_cmd)}", file=sys.stderr)

    try:
        result = subprocess.run(container_cmd, check=True, capture_output=True, text=True)
        print(f"Container created successfully: {container_name}", file=sys.stderr)
    except subprocess.CalledProcessError as e:
        print(f"Failed to create container: {e}", file=sys.stderr)
        if e.stderr:
            print(f"Container creation stderr: {e.stderr}", file=sys.stderr)
        raise


def execute_in_container(
    container_binary: str,
    container_name: str,
    claude_binary: str,
    claude_args: list,
):
    """Execute claude command in the existing container."""
    exec_cmd = [container_binary, "exec"]
    
    # Add -i flag for interactive use
    exec_cmd.append("-i")
    
    # Add -t flag only if running from a terminal
    if sys.stdin.isatty():
        exec_cmd.append("-t")
    
    exec_cmd.extend([container_name, claude_binary] + claude_args)

    print(f"Executing in container: {' '.join(exec_cmd)}", file=sys.stderr)

    try:
        result = subprocess.run(exec_cmd, check=False)
        return result.returncode
    except KeyboardInterrupt:
        print("\nInterrupted by user", file=sys.stderr)
        return 130
    except Exception as e:
        print(f"Error executing command in container: {e}", file=sys.stderr)
        return 1


def main():
    # Get environment variables
    base_image = os.environ.get(
        "CLAUDE_CONTAINER_IMAGE", "docker.io/nuhotetotniksvoboden/claudecodeui:latest"
    )
    claude_binary = os.environ.get("CLAUDE_BINARY", "claude")
    container_binary = os.environ.get("CONTAINER_BINARY", "podman")
    debug_mode = os.environ.get("DEBUG", "").lower() in ("true", "1", "yes")

    # Get directories
    home_dir = os.path.expanduser("~")
    current_dir = os.getcwd()

    try:
        # Parse command line arguments
        args, remaining_args = parse_args()
        
        # Get session ID and whether to add --session-id to claude args
        session_id, add_session_id = get_session_info(args)
        container_name = f"claude-session-{session_id}"
        
        # Prepare claude arguments - use ALL original arguments
        claude_args = sys.argv[1:].copy()
        if add_session_id:
            claude_args.extend(["--session-id", session_id])

        print(f"Using session ID: {session_id}", file=sys.stderr)
        print(f"Container name: {container_name}", file=sys.stderr)
        print(f"Add session-id to args: {add_session_id}", file=sys.stderr)

        # Create container if it doesn't exist
        if not container_exists(container_binary, container_name):
            create_container(container_binary, container_name, base_image, 
                           home_dir, current_dir, debug_mode)

        print(f"Container {container_name} already exists, reusing it", file=sys.stderr)

        # Execute claude command in the container
        exit_code = execute_in_container(container_binary, container_name, claude_binary,
                                       claude_args)
        sys.exit(exit_code)

    except KeyboardInterrupt:
        print("\nInterrupted by user", file=sys.stderr)
        sys.exit(130)
    except Exception as e:
        print(f"Unexpected error: {e}", file=sys.stderr)
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
