# Run

## From source

First build frontend

```sh
cd frontend
npm install
npm run build
```

Then build and run backend (will embed frontend files inside)

```sh
cargo build
cargo run
```

Then you can access it via ngrok
```sh
ngrok authtoken <your-token>
ngrok http 3000
```

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
    security_opt:
      - label:disable
      - seccomp:unconfined
    cap_add:
      - ALL
    environment:
      - HTTP_LISTEN_ADDRESS=0.0.0.0:3000
      - HOME=/home/dev
      # If running claude via podman
      # - CLAUDE_BINARY_PATH=/bin/claude-container
      # - CONTAINER_RUNTIME=podman
      # If you want to use socket
      # - CONTAINER_HOST=unix:///run/user/1934/podman/podman.sock
      # If will be similar for docker integration
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
