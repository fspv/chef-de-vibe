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


def is_git_repository(directory: str) -> bool:
    """Check if the directory is a git repository."""
    result = subprocess.run(
        ["git", "rev-parse", "--git-dir"],
        cwd=directory,
        capture_output=True,
        check=False
    )
    return result.returncode == 0


def create_git_worktree(session_id: str, source_dir: str, worktree_base_dir: str) -> str:
    """Create a git worktree for the given session if it doesn't exist.
    
    Returns the worktree directory path.
    """
    branch_name = session_id
    worktree_dir = os.path.join(worktree_base_dir, branch_name)
    
    # Check if worktree directory already exists
    if os.path.exists(worktree_dir):
        return worktree_dir
    
    # Create the worktree base directory if it doesn't exist
    os.makedirs(worktree_base_dir, exist_ok=True)
    
    # Create the worktree with a new branch in one command
    subprocess.run(
        ["git", "worktree", "add", "-b", branch_name, worktree_dir],
        cwd=source_dir,
        capture_output=True,
        check=True
    )
    
    print(f"Created git worktree: {worktree_dir}", file=sys.stderr)
    return worktree_dir


def parse_args():
    """Parse command line arguments to extract session-related flags."""
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--resume", type=str, help="Resume session with session ID")
    parser.add_argument("--session-id", type=str, help="Session ID")
    parser.add_argument("--dangerously-skip-permissions", action="store_true", 
                        help="Skip permission checks by auto-approving all requests")
    
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
                    home_dir: str, work_dir: str, target_dir: str, debug_mode: bool, 
                    skip_permissions: bool = False):
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
        "--security-opt", "label=disable",
        "--security-opt", "seccomp=unconfined", 
        "--cap-add=all",
        "--privileged",
        "-v", f"{home_dir}/.claude.json:/root/.claude.json:Z",
        "-v", f"{home_dir}/.claude/:/root/.claude/:Z",
        "-v", f"{work_dir}:{target_dir}:Z",
    ])

    # Add Claude settings override if skip_permissions is enabled
    if skip_permissions:
        container_cmd.extend([
            "-v", "/root/.claude/settings.json:/root/.claude/settings.json:Z",
            "-v", "/bin/claude-hook-pretooluse.sh:/bin/claude-hook-pretooluse.sh:Z"
        ])

    container_cmd.extend([
        "-w", target_dir,  # Set working directory inside container
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
    git_worktrees_dir = os.environ.get("GIT_WORKTREES_DIR", "/git-worktrees/")

    # Get directories
    home_dir = os.path.expanduser("~")
    current_dir = os.getcwd()

    try:
        # Parse command line arguments
        args, remaining_args = parse_args()
        
        # Get session ID and whether to add --session-id to claude args
        session_id, add_session_id = get_session_info(args)
        container_name = f"claude-session-{session_id}"
        
        # Check if we should skip permissions
        skip_permissions = args.dangerously_skip_permissions
        
        if skip_permissions:
            print("WARNING: Dangerously skip permissions is enabled. This will auto-approve all permission requests.", file=sys.stderr)
        
        # Handle git worktree creation if current directory is a git repo
        work_dir = current_dir
        if is_git_repository(current_dir):
            work_dir = create_git_worktree(session_id, current_dir, git_worktrees_dir)
        
        # Prepare claude arguments - use ALL original arguments except --dangerously-skip-permissions
        claude_args = []
        for arg in sys.argv[1:]:
            if arg != "--dangerously-skip-permissions":
                claude_args.append(arg)
        
        if add_session_id:
            claude_args.extend(["--session-id", session_id])

        print(f"Using session ID: {session_id}", file=sys.stderr)
        print(f"Container name: {container_name}", file=sys.stderr)
        print(f"Add session-id to args: {add_session_id}", file=sys.stderr)

        # Create container if it doesn't exist
        if not container_exists(container_binary, container_name):
            create_container(container_binary, container_name, base_image, 
                           home_dir, work_dir, current_dir, debug_mode, skip_permissions)

        print(f"Container {container_name} already exists, reusing it", file=sys.stderr)

        # Execute claude command in the container
        execute_in_container(container_binary, container_name, claude_binary, claude_args)
        sys.exit(0)

    except KeyboardInterrupt:
        print("\nInterrupted by user", file=sys.stderr)
        sys.exit(130)
    except Exception as e:
        print(f"Unexpected error: {e}", file=sys.stderr)
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
