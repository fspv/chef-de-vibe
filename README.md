⚠️ **CRITICAL SECURITY WARNING** ⚠️

**NEVER expose this application directly to the internet!** Chef De Vibe provides unrestricted command execution on your host machine. Exposing it publicly would give attackers complete control over your server. 

**Only access this application through:**
- Private VPN tunnels (e.g., Tailscale, WireGuard)
- SSH port forwarding
- Other secure, authenticated private networks

---

Claude in your pocket. Got a brilliant coding idea while away from your laptop? Don't wait - start coding immediately from your phone. Chef De Vibe runs persistent AI coding sessions on your server that you can access from anywhere - phone, tablet, laptop, or any browser. Never lose momentum on great ideas again.

Chef De Vibe is flexible enough to run Claude instances however you prefer - directly on your system for simplicity, or in containers for security and isolation. Need to protect your server from potential AI mishaps? The included [claude-container](/claude-container) wrapper makes containerized execution effortless.

https://github.com/user-attachments/assets/26104cc3-6043-49bb-9aec-c6bb9aa2d9a7

# Run

## Using Brew

```sh
brew install fspv/apps/chef-de-vibe
chef-de-vibe
```

## Using Nix

```sh
# Clone and run
git clone https://github.com/fspv/chef-de-vibe
cd chef-de-vibe
nix run

# Or run directly from GitHub (if flake supports it)
nix run github:fspv/chef-de-vibe
```

# From precompiled binaries

Precompiled binaries for different platforms are available on the releases
page.

## Using docker compose

```yaml
services:
  chef-de-vibe:
    image: nuhotetotniksvoboden/chef-de-vibe:latest
    container_name: chef-de-vibe
    privileged: true
    restart: unless-stopped
    volumes:
      - /home/dev:/home/dev
      # if you want to use podman via socket
      # - /run/user/1934/podman/podman.sock:/run/user/1934/podman/podman.sock
    environment:
      - HTTP_LISTEN_ADDRESS=0.0.0.0:3000
      - HOME=/home/dev
      # If running claude via podman
      # - CLAUDE_BINARY_PATH=/bin/claude-container
      # - CONTAINER_RUNTIME=podman
      # If you want to use socket
      # - CONTAINER_HOST=unix:///run/user/1934/podman/podman.sock
      # It will be similar for docker integration
      # To make git and podman socket work in the container
      # - CONTAINER_ARGS=-e CONTAINER_HOST=unix:///run/user/1934/podman/podman.sock -v /home/dev/.gitconfig:/root/.gitconfig -v /run/user/1934/podman/podman.sock:/run/user/1934/podman/podman.sock
  # If you want to access it via tailscale
  #   network_mode: service:tailscale_chefdevibe
  #   depends_on:
  #     - tailscale_chefdevibe
  # tailscale_chefdevibe:
  #   image: tailscale/tailscale:latest
  #   container_name: tailscale_chefdevibe
  #   hostname: chef-de-vibe
  #   env_file:
  #     - .env
  #   environment:
  #     - TS_AUTH_KEY=
  #     - TS_STATE_DIR=/var/lib/tailscale
  #     - TS_USERSPACE=true
  #     - TS_ACCEPT_DNS=true
  #     - TS_EXTRA_ARGS=
  #   volumes:
  #     - ./tailscale/chefdevibe/state:/var/lib/tailscale
  #     - /dev/net/tun:/dev/net/tun
  #     - ./tailscale/config:/config
  #   restart: unless-stopped
```

## From source

First build frontend

```sh
cd frontend
npm install
npm run build
```

Then build and run backend (will embed frontend files inside)

```sh
cargo run
```

Then you can access it via:
- Tailscale (recommended for private access)
- ngrok (use with authentication)
- Cloudflare Tunnel (use with authentication)
