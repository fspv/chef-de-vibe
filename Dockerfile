ARG CLAUDECODEUI_VERSION=latest
FROM nuhotetotniksvoboden/claudecodeui:${CLAUDECODEUI_VERSION}

RUN apk add podman podman-compose fuse-overlayfs cni-plugins shadow iptables

RUN echo "$(whoami):100000:65536" | tee -a /etc/subuid
RUN echo "$(whoami):100000:65536" | tee -a /etc/subgid

COPY claude-container.py /usr/local/bin/claude
RUN chmod +x /usr/local/bin/claude

COPY claude-hook-pretooluse.sh /bin/claude-hook-pretooluse.sh
RUN chmod +x /bin/claude-hook-pretooluse.sh
