# Use official Playwright image with browsers pre-installed
FROM mcr.microsoft.com/playwright:v1.55.0-noble

# Set working directory
WORKDIR /tests

# Copy package files
COPY e2e/package*.json ./

# Install dependencies
RUN npm ci

# Copy test files
COPY e2e/ ./

# Set environment variable to indicate we're running in Docker
ENV DOCKER_TEST=1

# Run tests
CMD ["npx", "playwright", "test"]