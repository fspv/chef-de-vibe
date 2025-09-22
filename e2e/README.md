# E2E Tests for Chef de Vibe

End-to-end tests using Playwright to test the Chef de Vibe application through the browser.

## Architecture

The E2E tests run in a fully containerized environment:
- **App container**: Runs the Chef de Vibe application (backend + embedded frontend)
- **Playwright container**: Runs the Playwright tests against the app container

## Running Tests Locally

### Using Make (Recommended)
```bash
# Run all E2E tests
make e2e-test

# Build containers without running tests
make e2e-build

# Clean up containers and test results
make e2e-clean
```

### Using Podman Compose directly
```bash
# Run tests
podman-compose -f docker-compose.e2e.yml up --build --exit-code-from playwright

# Clean up
podman-compose -f docker-compose.e2e.yml down -v
```

## Test Structure

- `tests/` - Test specifications
  - `new-chat.spec.ts` - Tests for creating new chat sessions
- `fixtures/` - Page Object Models
  - `NewChatDialog.ts` - Interactions with new chat dialog
  - `SessionList.ts` - Interactions with session list
  - `ChatWindow.ts` - Interactions with chat window

## What's Tested

1. **New Chat Creation Flow**
   - Opening the new chat dialog
   - Filling in working directory and message
   - Handling API key validation errors
   - Verifying session appears in the list

## Test Results

After running tests, results are available in:
- `test-results/` - Test execution results and screenshots/videos on failure
- `playwright-report/` - HTML test report

## CI/CD

Tests automatically run in GitHub Actions on:
- Push to master/main branch
- Pull requests
- Manual workflow dispatch

Test artifacts are uploaded on failure for debugging.