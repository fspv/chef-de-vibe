.PHONY: help build test e2e-test e2e-build e2e-clean lint

help:
	@echo "Available targets:"
	@echo "  build       - Build the application"
	@echo "  test        - Run unit tests"
	@echo "  e2e-test    - Run E2E tests in Docker"
	@echo "  e2e-build   - Build E2E test containers"
	@echo "  e2e-clean   - Clean up E2E test containers"
	@echo "  lint        - Run linters"

# Build the application
build:
	cd frontend && npm ci && npm run build
	cargo build --release

# Run unit tests
test:
	cargo test -- --test-threads=1

# Build E2E test containers
e2e-build:
	podman-compose --podman-build-args "--security-opt seccomp=unconfined" -f docker-compose.e2e.yml build

# Run E2E tests
e2e-test:
	@echo "Building and running E2E tests..."
	podman-compose --podman-build-args "--security-opt seccomp=unconfined" -f docker-compose.e2e.yml up --build --exit-code-from playwright
	@echo "E2E tests completed. Check test-results/ for reports."

# Clean up E2E test containers and volumes
e2e-clean:
	podman-compose -f docker-compose.e2e.yml down -v
	rm -rf test-results playwright-report

# Run linters
lint:
	cd frontend && npm run lint
	cargo clippy --all-targets --all-features -- -D warnings