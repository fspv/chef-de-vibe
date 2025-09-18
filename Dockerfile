# Use the specified base image
FROM nuhotetotniksvoboden/claudecodeui:latest AS base

# Frontend build stage
FROM base AS frontend-builder
WORKDIR /app/frontend

# Copy package files
COPY frontend/package.json frontend/package-lock.json ./

# Install frontend dependencies (including dev dependencies for build)
RUN npm ci --include=dev

# Copy frontend source
COPY frontend/ .

# Build frontend
RUN npm run build

# Backend build stage
FROM base AS backend-builder
WORKDIR /app

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ ./src/

# Copy built frontend from frontend-builder stage
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist

# Build the backend
RUN cargo build --release

# Final runtime stage
FROM base AS runtime
WORKDIR /app

# Install podman and podman-compose
RUN apk add --no-cache \
    podman \
    python3 \
    py3-pip \
    curl && \
    pip3 install --break-system-packages podman-compose

# Copy the built binary
COPY --from=backend-builder /app/target/release/chef-de-vibe ./

# Copy the claude-container script to /bin
COPY claude-container /bin/claude-container
RUN chmod +x /bin/claude-container

# Set the binary as executable and run it
RUN chmod +x ./chef-de-vibe
CMD ["./chef-de-vibe"]
