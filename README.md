# Chef de Vibe - Claude Code UI, but with every chat running in its own container

This project is a wrapper around https://github.com/siteboon/claudecodeui which runs a podman container for every new session with only the project directory mounted.

This allows to reduce the blast radius to a single project if things go not the way you have expected, giving you the peace of mind and an ability to unlock the full vibe mode.

Note: `--dangerously-skip-permissions` flag would not work, because claude will be running as root.

## Quick Start

### Using Docker/Podman (Recommended)

Run the container with the following command:

```bash
podman run \
  --name chef-de-vibe --replace --privileged \
  -p 3001:3001 \
  -e HOME=${HOME} \
  -v ${HOME}:${HOME} \
  --restart unless-stopped \
  nuhotetotniksvoboden/chef-de-vibe:latest
```

Or with Docker:

```bash
docker run \
  --name chef-de-vibe --replace --privileged \
  -p 3001:3001 \
  -e HOME=${HOME} \
  -v ${HOME}:${HOME} \
  --restart unless-stopped \
  nuhotetotniksvoboden/chef-de-vibe:latest
```

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
      - ${HOME}:${HOME}
    environment:
      - CLAUDE_CONTAINER_IMAGE=${CLAUDE_CONTAINER_IMAGE:-nuhotetotniksvoboden/claudecodeui:latest}
      - CLAUDE_BINARY=${CLAUDE_BINARY:-claude}
      - CONTAINER_BINARY=${CONTAINER_BINARY:-podman}
      - DEBUG=${DEBUG:-false}
```

Then run:

```bash
docker-compose up -d
```

Or with Podman Compose:

```bash
podman-compose up -d
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CLAUDE_CONTAINER_IMAGE` | `nuhotetotniksvoboden/claudecodeui:latest` | Base image to use for running Claude commands |
| `CLAUDE_BINARY` | `claude` | Path to the Claude binary inside the container |
| `CONTAINER_BINARY` | `podman` | Container runtime to use (podman/docker) |
| `DEBUG` | `false` | Enable debug mode (true/1/yes to enable) |

## Volume Mounts

The container requires several volume mounts to function properly:

- `${HOME}/.claude.json:/root/.claude.json` - Claude configuration file
- `${HOME}/.claude/.credentials.json:/root/.claude/.credentials.json` - Claude credentials
- `${HOME}/.claude/projects:/root/.claude/projects` - Claude projects directory
- `${HOME}:${HOME}` - Full home directory access for project files

## Networking

The container exposes port 3001 internally, which is mapped to port 3333 on the host. Access the Claude Code UI at:

http://localhost:3333

## Building from Source

To build the container image:

```bash
podman build -t chef-de-vibe .
```

Or with Docker:

```bash
docker build -t chef-de-vibe .
```

## Claude Container Wrapper

The project includes a Python wrapper script (`claude-container.py`) that provides a seamless way to run Claude commands inside containers. The wrapper:

- Automatically mounts necessary volumes
- Preserves working directory context
- Handles terminal interaction properly
- Supports debug mode for troubleshooting

## Requirements

- Docker or Podman
- Claude configuration files in `${HOME}/.claude/`
- Sufficient privileges for container operations (--privileged flag)

## Security Considerations

This container runs in privileged mode and mounts the host's home directory. This is necessary for Claude Code UI to access project files but should be used with appropriate security considerations in mind.

## Troubleshooting

### Debug Mode

Enable debug mode to see detailed command output:

```bash
DEBUG=true podman run ...
```

### Container Runtime Issues

If you encounter issues with Podman, try switching to Docker by setting:

```bash
CONTAINER_BINARY=docker
```

### Permission Issues

Ensure your user has proper permissions to run containers and access the mounted directories.
