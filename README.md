# Chef de Vibe - Claude Code UI, but with every chat running in its own container

This project is a wrapper around https://github.com/siteboon/claudecodeui which runs a podman container for every new session with only the project directory mounted.

This allows to reduce the blast radius to a single project if things go not the way you have expected, giving you the peace of mind and an ability to unlock the full vibe mode.

**Git Worktree Integration**: For git repositories, each session automatically creates an isolated git worktree with its own branch (named after the session ID). This ensures complete isolation between different Claude sessions while preserving git history and allowing independent development workflows.

Note: `--dangerously-skip-permissions` flag would not work, because claude will be running as root.

## Quick Start

### Using Rootless Podman

Run the container with the following command:

```bash
podman run \
  --name chef-de-vibe --replace --privileged \
  -p 3001:3001 \
  -e HOME=${HOME} \
  -v ${HOME}:${HOME} \
  -v ./git-worktrees:/git-worktrees \
  --restart unless-stopped \
  nuhotetotniksvoboden/chef-de-vibe:latest
```

You can use docker as well, but then you need to somehow make sure the process runs from the same user as your user, not root. Otherwise it will mess up file permissions and it will be hard to work with it.

### Using Docker Compose

Create a `docker-compose.yml` file:

```yaml
services:
  chef-de-vibe:
    image: nuhotetotniksvoboden/chef-de-vibe:latest
    container_name: chef-de-vibe
    privileged: true
    restart: unless-stopped
    ports:
      - "3001:3001"
    volumes:
      # Home should contain your .claude dir, .claude.json files and all the
      # repositories referenced in the sessions
      - ${HOME}:${HOME}
      - ./git-worktrees:/git-worktrees
    environment:
      # Need to set home variable to the same as your current home, otherwise paths will be messed up
      - HOME=${HOME}
      - CLAUDE_CONTAINER_IMAGE=${CLAUDE_CONTAINER_IMAGE:-docker.io/nuhotetotniksvoboden/claudecodeui:latest}
      - CLAUDE_BINARY=${CLAUDE_BINARY:-claude}
      - CONTAINER_BINARY=${CONTAINER_BINARY:-podman}
      - DEBUG=${DEBUG:-false}
      - GIT_WORKTREES_DIR=${GIT_WORKTREES_DIR:-/home/dev/git/worktrees/}
```

Then run:

```bash
docker-compose up -d
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CLAUDE_CONTAINER_IMAGE` | `nuhotetotniksvoboden/claudecodeui:latest` | Base image to use for running Claude commands |
| `CLAUDE_BINARY` | `claude` | Path to the Claude binary inside the container |
| `CONTAINER_BINARY` | `podman` | Container runtime to use (podman/docker) |
| `DEBUG` | `false` | Enable debug mode (true/1/yes to enable) |
| `GIT_WORKTREES_DIR` | `/git-worktrees/` | Directory to store git worktrees for session isolation |
