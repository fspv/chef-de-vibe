# Chef de Vibe Service - Integrated Technical Design

## 1. Overview

Chef de Vibe is an HTTP service that manages multiple Claude Code instances, providing WebSocket-based access to their stdin/stdout streams. Each instance is uniquely identified by a session ID. The service can discover existing sessions from disk and manage both active and inactive sessions.

## 2. Architecture Components

### 2.1 Main Components
- **HTTP Server**: RESTful API for session management and listing
- **WebSocket Server**: Real-time bidirectional communication with Claude instances
- **Approval WebSocket Server**: Separate WebSocket endpoint for tool usage approval requests
- **Session Manager**: Orchestrates Claude process lifecycle
- **Approval Manager**: Handles tool permission requests and responses per session
- **Session Discovery Service**: Scans disk for existing session files
- **Background Worker Pool**: Handles session creation asynchronously but with synchronous API
- **Claude Code Processes**: Child processes running `claude` command with permission prompting enabled

### 2.2 State Management
In-memory state:
- Map of session_id → Session metadata
- Session metadata includes:
  - Process ID
  - Working directory
  - WebSocket clients list (connection objects)
  - Approval WebSocket clients list (separate from main clients)
  - Write queue (FIFO queue of pending writes)
  - Approval requests queue (pending tool permission requests)
  - Creation status (pending/ready/failed)
  - Background worker reference

Persistent state on disk:
- Session files stored in project directories under `CLAUDE_PROJECTS_DIR`
- Each session stored as `{session-id}.jsonl` file
- Files contain JSONL formatted session history

## 3. Configuration

### 3.1 Environment Variables
| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `CLAUDE_BINARY_PATH` | Full path to claude executable | Yes | - |
| `HTTP_LISTEN_ADDRESS` | Address:port for HTTP/WS server | No | `127.0.0.1:3000` |
| `CLAUDE_PROJECTS_DIR` | Directory where Claude stores project sessions | No | `~/.claude/projects` |
| `SHUTDOWN_TIMEOUT` | Seconds to wait for graceful shutdown | No | 30 |

### 3.2 Startup Validation
1. Verify `CLAUDE_BINARY_PATH` exists and is executable
   - If not → **CRASH** with error message
2. Verify `CLAUDE_PROJECTS_DIR` exists and is readable
   - If not → **CRASH** with error message
3. Start HTTP server on `HTTP_LISTEN_ADDRESS`
   - If fails → **CRASH** with error message
4. Initialize background worker pool

## 4. API Specifications

### 4.1 HTTP Endpoints

#### 4.1.1 GET /api/v1/sessions - List All Sessions
**Response (200 OK):**
```json
{
  "sessions": [
    {
      "session_id": "4d02fe0a-7c6d-4cf9-967a-92391f73b6aa",
      "working_directory": "/home/user/project1",
      "active": true,
      "summary": "API Endpoint Refactoring: Standardizing Routes",
      "earliest_message_date": "2025-09-12T16:19:40.665Z",
      "latest_message_date": "2025-09-12T16:20:01.786Z"
    },
    {
      "session_id": "619a17f0-e65b-4f2f-8260-a62bc8087709",
      "working_directory": "/home/dev",
      "active": false,
      "summary": "Enhancing CLAUDE.md with DevOps and Best Practices",
      "earliest_message_date": "2025-09-10T08:30:15.123Z",
      "latest_message_date": "2025-09-10T09:45:22.456Z"
    }
  ]
}
```

**Note**: The `summary`, `earliest_message_date`, and `latest_message_date` fields are optional and will only be present if:
- `summary`: A summary entry with `"type":"summary"` exists in the session's journal file
- `earliest_message_date`/`latest_message_date`: Message entries with timestamps exist in the session's journal file

Sessions without summaries or timestamps will omit these fields from the response.

**Error Response:**
```json
{
  "error": "Human-readable error message",
  "code": "ERROR_CODE"
}
```

**Error Codes:**
- `DIRECTORY_READ_ERROR`: Failed to read projects directory
- `FILE_PARSE_ERROR`: Failed to parse session file
- `INTERNAL_ERROR`: Unexpected orchestrator error

#### 4.1.2 POST /api/v1/sessions - Create or Resume Session
**Request:**
```json
{
  "session_id": "unique-session-identifier",
  "working_dir": "/absolute/path/to/project",
  "resume": true | false,
  "first_message": ["message1", "message2", ...]
}
```

**Note about first_message field:**
- `first_message` is required and must be an array of strings
- Each string contains a raw JSON message that will be forwarded directly to Claude's stdin
- Messages are sent in order, with each message on a separate line
- All JSON messages are automatically compacted to single-line format before being sent to Claude, as Claude expects each JSON message to be on a single line

**Example:**
```json
{
  "session_id": "my-session",
  "working_dir": "/home/user/project",
  "resume": false,
  "first_message": [
    "{\"role\": \"user\", \"content\": \"Hello Claude\"}",
    "{\"role\": \"user\", \"content\": \"Please help me with this project\"}"
  ]
}
```

**Response (200 OK):**
```json
{
  "session_id": "actual-session-id",
  "websocket_url": "/api/v1/sessions/actual-session-id/claude_ws",
  "approval_websocket_url": "/api/v1/sessions/actual-session-id/claude_approvals_ws"
}
```
Note: `session_id` in response may differ from request when `resume: true`

**Error Response:**
```json
{
  "error": "Human-readable error message",
  "code": "ERROR_CODE"
}
```

**Error Codes:**
- `INVALID_REQUEST`: Malformed JSON or missing required fields (session_id, working_dir, resume, first_message)
- `WORKING_DIR_INVALID`: Working directory doesn't exist or isn't accessible
- `CLAUDE_SPAWN_FAILED`: Failed to spawn Claude process
- `INTERNAL_ERROR`: Unexpected orchestrator error

#### 4.1.3 GET /api/v1/sessions/{session_id} - Check Session Status
**Response (200 OK) - Session exists and running:**
```json
{
  "session_id": "session-123",
  "working_directory": "/home/user/project",
  "content": [
    {"type": "user", "message": {"role": "user", "content": "Hello"}},
    {"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Hi there!"}]}}
  ],
  "websocket_url": "/api/v1/sessions/session-123/claude_ws",
  "approval_websocket_url": "/api/v1/sessions/session-123/claude_approvals_ws"
}
```

**Response (200 OK) - Session exists but not running:**
```json
{
  "session_id": "session-123",
  "working_directory": "/home/user/project",
  "content": [
    {"type": "user", "message": {"role": "user", "content": "Hello"}},
    {"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Hi there!"}]}}
  ]
}
```
Note: No `websocket_url` or `approval_websocket_url` fields when session is not running

**Response (404 Not Found):**
```json
{
  "error": "Session not found",
  "code": "SESSION_NOT_FOUND"
}
```

**Error Response (400 Bad Request):**
```json
{
  "error": "Failed to parse session file",
  "code": "FILE_PARSE_ERROR"
}
```

### 4.2 WebSocket Endpoint

#### 4.2.1 Endpoint Path
`/api/v1/sessions/{session_id}/claude_ws`

#### 4.2.2 Message Format
- **Client → Server**: Raw JSON as expected by Claude
- **Server → Client**: Raw JSON from Claude OR echoed input from other clients
- All messages are text frames containing JSON

#### 4.2.3 Connection Behavior
- Multiple clients can connect simultaneously
- New clients receive only messages generated after connection
- No replay of buffered messages
- Connection is refused if session doesn't exist

### 4.3 Tool Approval WebSocket Endpoint

#### 4.3.1 Endpoint Path
`/api/v1/sessions/{session_id}/claude_approvals_ws`

#### 4.3.2 Purpose
Separate WebSocket endpoint for handling Claude's tool usage approval requests. When Claude is configured with tool permissions enabled, it sends control requests asking for permission to use specific tools before executing them.

#### 4.3.3 Message Format

The backend acts as a pass-through proxy, wrapping Claude's raw approval requests with unique IDs and forwarding responses back to Claude without parsing the message content.

**Server → Client Messages:**

All approval requests (both pending and new) are sent as individual messages with the same format. When a client connects, the backend sends each pending approval as a separate message.

**Approval Request** (sent for both pending and new requests):
```json
{
  "id": "uuid-1234",
  "request": {
    // Raw Claude approval request - backend passes through as-is
    "tool_name": "Read",
    "input": {
      "file_path": "/etc/passwd"
    },
    "permission_suggestions": [
      {
        "type": "addRules",
        "rules": [{"toolName": "Read", "ruleContent": "/etc/*"}],
        "behavior": "allow",
        "destination": "session"
      }
    ]
  },
  "created_at": "2024-01-01T10:05:00Z"
}
```

**Client → Server Messages:**

**Approval Response**:
```json
{
  "id": "uuid-1234",
  "response": {
    // Raw Claude approval response format - backend forwards as-is to Claude
    "behavior": "allow",
    "updatedInput": {
      "file_path": "/etc/passwd"
    },
    "updatedPermissions": [
      {
        "type": "addRules",
        "rules": [{"toolName": "Read", "ruleContent": "/etc/*"}],
        "behavior": "allow", 
        "destination": "session"
      }
    ]
  }
}
```

**Backend Behavior:**
- Backend does not parse or validate the contents of `request` or `response` fields
- Backend only validates that messages are valid JSON with required `id` field
- All message parsing and construction is handled by Claude and the frontend directly

#### 4.3.4 Connection Behavior
- Multiple approval clients can connect simultaneously
- New clients immediately receive all pending approval requests for the session
- Approval requests persist in memory until explicitly approved/denied
- If all approval clients disconnect, requests remain pending until reconnection
- Connection is refused if session doesn't exist or is not active
- When Claude process dies, all pending approvals for that session are cleared

#### 4.3.5 Approval Request Lifecycle
1. Claude sends `control_request` with `can_use_tool` subtype to stdin/stdout
2. Server detects control request in Claude output stream
3. Server generates unique `id` and wraps Claude's raw request in message envelope
4. Server stores wrapped request in session approval state
5. Server broadcasts wrapped approval request to all connected approval WebSocket clients
6. Client user reviews request and sends response with matching `id`
7. Server extracts response content and forwards as `control_response` to Claude's stdin
8. Server removes request from pending state
9. Claude continues execution based on approval decision

## 5. Session Discovery and File Operations

### 5.1 Session File Structure
Session files are stored in project-specific directories under `CLAUDE_PROJECTS_DIR`:
- Directory structure: `{CLAUDE_PROJECTS_DIR}/{project-dir}/{session-id}.jsonl`
- Each file contains JSONL formatted entries
- Each line is a JSON object with session metadata and messages

### 5.2 Session Discovery Algorithm
For listing all sessions:
1. Recursively scan all subdirectories under `CLAUDE_PROJECTS_DIR`
2. For each `.jsonl` file found:
   - Extract session ID from filename
   - Parse file line by line until both `sessionId` and `cwd` fields are found
   - Validate that filename session ID matches the `sessionId` field in content
   - If mismatch → skip and log error
   - Add to session list with working directory
3. Check in-memory session map to mark active sessions
4. Return combined list

### 5.3 Finding Specific Session
For GET /sessions/{session_id}:
1. Check in-memory session map first
2. If not found, scan all project directories:
   - Look for file named `{session_id}.jsonl`
   - Parse and validate file
   - Return content
3. If not found anywhere → return 404

### 5.4 File Parsing Rules
- Files must be valid JSONL format
- Each line must be valid JSON
- Continue parsing until both `sessionId` and `cwd` are found
- If corrupted or invalid JSON → return `FILE_PARSE_ERROR`
- Session ID in filename must match `sessionId` field in content

## 6. Detailed User Journeys

### 6.1 Journey: List All Sessions

**Scenario**: Client wants to see all available sessions

**Steps**:

1. **Client sends** GET request to `/api/v1/sessions`

2. **Server scans** disk for sessions:
   - Iterate through all subdirectories in `CLAUDE_PROJECTS_DIR`
   - Find all `.jsonl` files
   - Parse each file to extract session ID and working directory

3. **Server checks** active sessions:
   - Compare found sessions with in-memory session map
   - Mark which sessions have running Claude processes

4. **Server returns** complete list:
   - All sessions from disk
   - Active status for each session

**Edge Cases**:
- **Empty projects directory**: Return empty sessions array
- **Corrupted file**: Skip session file and log error
- **Missing cwd field**: Skip session or return error depending on policy
- **Session ID mismatch**: Skip session file and log error

### 6.2 Journey: Create New Session

**Scenario**: Client wants to start a new Claude Code session

**Steps**:

1. **Client sends** POST request to `/api/v1/sessions`:
   ```json
   {
     "session_id": "session-123",
     "working_dir": "/home/user/project",
     "resume": false,
     "first_message": "{\"role\": \"user\", \"content\": \"Hello\"}"
   }
   ```

2. **Server validates** request:
   - Parse JSON (if fails → return 400 with `INVALID_REQUEST`)
   - Check required fields present (session_id, working_dir, resume, first_message)
   - Check if session already exists in memory
     - If exists and running → return 200 with existing WebSocket URL immediately
     - If exists but not running → continue to step 3

3. **Server creates** background task:
   - Create session entry with status `pending`
   - Spawn background worker
   - **Important**: Server waits for worker completion (synchronous from client perspective)

4. **Background worker** executes:
   - Spawn Claude process with specified working directory and `--session-id <session id>` and `--output-format stream-json --input-format stream-json --verbose --print` flags
   - Send first_message raw JSON to Claude stdin
   - Wait for Claude's first response to confirm session is ready
   - Update session status to `ready`
   - Store process reference in session map

5. **Server returns** response to waiting client:
   ```json
   {
     "session_id": "session-123",
     "websocket_url": "/sessions/session-123/claude_ws",
     "approval_websocket_url": "/sessions/session-123/claude_approvals_ws"
   }
   ```

### 6.3 Journey: Resume Existing Session

**Scenario**: Client wants to resume a previous Claude session

**Steps**:

1. **Client sends** POST request:
   ```json
   {
     "session_id": "old-session-456",
     "working_dir": "/home/user/project",
     "resume": true,
     "first_message": "{\"role\": \"user\", \"content\": \"Resume session\"}"
   }
   ```

2. **Server creates** background task

3. **Background worker** spawns Claude with `--resume <session id>` and `--output-format stream-json --input-format stream-json --verbose --print` flags using provided working directory

4. **Worker sends first_message** raw JSON to Claude stdin

5. **Worker waits for Claude's first response** to get new session ID

6. **Worker updates** session tracking:
   - Creates new session entry for new session ID
   - Removes placeholder for old session ID

7. **Server returns** with NEW session ID:
   ```json
   {
     "session_id": "new-session-789",
     "websocket_url": "/sessions/new-session-789/claude_ws",
     "approval_websocket_url": "/sessions/new-session-789/claude_approvals_ws"
   }
   ```

### 6.4 Journey: Get Session from Disk

**Scenario**: Client requests information about an inactive session

**Steps**:

1. **Client sends** GET request to `/api/v1/sessions/abc-123`

2. **Server checks** in-memory session map
   - Not found → proceed to disk search

3. **Server searches** disk:
   - Scan all project directories for `abc-123.jsonl`
   - Found in `/projects/some-project/abc-123.jsonl`

4. **Server parses** session file:
   - Read file line by line
   - Parse each line as JSON
   - Collect all message entries

5. **Server returns** session content without WebSocket URL

### 6.5 Journey: Multiple WebSocket Clients

**Scenario**: Multiple clients connect to same session

**Steps**:

1. **Client A** connects to `/api/v1/sessions/session-123/claude_ws`
   - Server accepts connection
   - Adds Client A to session's client list

2. **Client B** connects to same WebSocket endpoint
   - Server accepts connection
   - Adds Client B to session's client list

3. **Client A** sends message
   - Server adds to write queue
   - Writes to Claude stdin
   - Broadcasts to Client B only (not back to Client A)

4. **Claude responds**
   - Server broadcasts response to both Client A and Client B

### 6.6 Journey: Session with No Connected Clients

**Scenario**: All WebSocket clients disconnect

**Steps**:

1. **Last client disconnects** from WebSocket
2. **Server continues** processing:
   - Keep reading from Claude stdout
   - **Discard all output** (no buffering)
   - Keep Claude process running
   - Write queue continues processing

3. **New client connects** later:
   - Receives only new output from that point
   - No replay of missed messages

### 6.7 Journey: Claude Process Dies

**Scenario**: Claude process terminates unexpectedly

**Steps**:

1. **Server detects** process exit
2. **Server broadcasts** disconnect to all WebSocket clients
3. **WebSocket connections** are closed with status 1011 (internal error)
4. **Session removed** from active sessions map
5. **GET requests** will now return without WebSocket URL

### 6.8 Journey: Graceful Shutdown

**Scenario**: Server receives shutdown signal

**Steps**:

1. **Signal received** (SIGTERM/SIGINT)
2. **Stop accepting** new HTTP connections
3. **Close all** WebSocket connections with status 1001 (going away)
4. **Send SIGTERM** to all Claude processes
5. **Wait up to** `SHUTDOWN_TIMEOUT` seconds
6. **Send SIGKILL** to remaining processes
7. **Exit** with code 0

### 6.9 Journey: Tool Approval Request Flow

**Scenario**: Claude attempts to use a tool and requires user approval

**Steps**:

1. **Client connects** to approval WebSocket `/api/v1/sessions/session-123/claude_approvals_ws`

2. **Server sends** pending approvals (if any exist) as individual approval request messages

3. **Claude attempts** to use a tool (e.g., Read file):
   - Claude sends control_request via stdout to server with format:
     ```json
     {
       "type": "control_request",
       "request_id": "060c666b-b430-404f-9176-99b27d76f81c",
       "request": {
         "subtype": "can_use_tool",
         "tool_name": "Bash",
         "input": {"command": "curl google.com", "description": "Fetch Google homepage"}
       }
     }
     ```

4. **Server processes** control request:
   - Parses control_request from Claude stdout
   - Generates unique request_id 
   - Stores request in session approval state
   - Does NOT forward to regular claude_ws clients

5. **Server wraps and broadcasts** approval request to approval WebSocket clients:
   ```json
   {
     "id": "uuid-1234",
     "request": {
       // Raw Claude request.request content - server passes through without parsing
       "subtype": "can_use_tool",
       "tool_name": "Bash",
       "input": {"command": "curl google.com", "description": "Fetch Google homepage"},
       "permission_suggestions": [...]
     },
     "created_at": "2024-01-01T10:00:00Z"
   }
   ```

6. **Client displays** approval dialog to user showing:
   - Tool name and purpose
   - Input parameters (formatted/highlighted)
   - Optional permission suggestions
   - Approve/Deny buttons

7. **User makes decision** and client sends response:
   ```json
   {
     "id": "uuid-1234",
     "response": {
       // Raw Claude response format - server forwards without parsing
       "behavior": "allow",
       "updatedInput": {"file_path": "/home/user/secret.txt"},
       "updatedPermissions": []
     }
   }
   ```

8. **Server processes** approval response:
   - Extracts `id` from client message to match pending request
   - Removes request from pending state  
   - Automatically sets request_id from the original Claude request
   - Forwards client's raw response content to Claude stdin as control_response:
     ```json
     {
       "type": "control_response",
       "response": {
         "subtype": "success",
         "request_id": "060c666b-b430-404f-9176-99b27d76f81c",
         "response": {/* client's raw response object - passed through unchanged */}
       }
     }
     ```

9. **Claude receives** approval and continues execution with tool usage

**Edge Cases**:
- **Client disconnects**: Approval request remains pending until reconnection
- **Multiple clients**: All approval clients see the same requests
- **Claude timeout**: Claude may have its own timeout for approval responses

### 6.10 Journey: Approval Request Denial

**Scenario**: User denies Claude's tool usage request

**Steps**:

1. **Same steps 1-6** as approval flow above

2. **User denies** request and client sends:
   ```json
   {
     "id": "uuid-1234",
     "response": {
       // Raw Claude denial format - server forwards as-is
       "behavior": "deny",
       "message": "Tool usage denied by user"
     }
   }
   ```

3. **Server processes** denial:
   - Removes request from pending state
   - Sends control_response to Claude with denial

4. **Claude receives** denial and either:
   - Continues without using the tool
   - Asks user for alternative approach
   - Reports inability to complete task

### 6.11 Journey: Multiple Pending Approvals

**Scenario**: Multiple tool usage requests accumulate while user is away

**Steps**:

1. **Claude sends** multiple control_requests while no approval clients connected:
   - Request 1: Read /etc/passwd  
   - Request 2: Write to /tmp/output.txt
   - Request 3: Execute bash command

2. **Server accumulates** requests in session approval state:
   - Each gets unique request_id
   - All remain in pending status
   - Claude processes block waiting for responses

3. **User connects** to approval WebSocket

4. **Server immediately sends** all pending requests as individual messages:
   ```json
   // Message 1:
   {
     "id": "uuid-1",
     "request": {/* Raw Claude request for Read tool */},
     "created_at": "..."
   }
   // Message 2:
   {
     "id": "uuid-2", 
     "request": {/* Raw Claude request for Write tool */},
     "created_at": "..."
   }
   // Message 3:
   {
     "id": "uuid-3",
     "request": {/* Raw Claude request for Bash tool */}, 
     "created_at": "..."
   }
   ```

5. **User reviews** and approves/denies each request

6. **Server processes** each response and forwards to Claude

7. **Claude processes** continue execution based on approval decisions

## 7. Write Queue Management

### 7.1 Queue Structure
Each session maintains a FIFO write queue containing:
- Message to be written
- Sender client ID
- Timestamp of enqueue

### 7.2 Queue Processing
- Messages are processed in strict FIFO order
- Only one message is written to Claude stdin at a time
- No waiting for Claude response before processing next item
- If Claude process dies, queue is cleared

### 7.3 Broadcast Logic
When client sends a message:
- Add to session's write queue
- Broadcast to all OTHER connected clients (not sender)
- Write to Claude stdin when queue position reached
- Claude's response is broadcast to ALL clients

## 8. Data Flow Patterns

### 8.1 Client Input Flow
```
Client A sends message
    ↓
WebSocket Server receives
    ↓
Add to session write queue
    ↓
Broadcast to Clients B, C, D (not A)
    ↓
Write to Claude stdin (when queue position reached)
```

### 8.2 Claude Output Flow
```
Claude stdout
    ↓
Server reads line
    ↓
Parse as JSON (validate)
    ↓
Broadcast to ALL connected clients (A, B, C, D)
    ↓
If no clients connected → Discard
```

### 8.3 Session Discovery Flow
```
Request for session list
    ↓
Scan CLAUDE_PROJECTS_DIR recursively
    ↓
Find all .jsonl files
    ↓
Parse each file for sessionId and cwd
    ↓
Check in-memory map for active status
    ↓
Return combined list
```

## 9. Error Handling Matrix

| Error Condition | Detection Point | Response | Recovery |
|-----------------|-----------------|----------|----------|
| Invalid JSON in HTTP request | Request parsing | HTTP 400 with `INVALID_REQUEST` | None |
| Missing required field | Request validation | HTTP 400 with `INVALID_REQUEST` | None |
| Session file not found | GET /sessions/{id} | HTTP 404 with `SESSION_NOT_FOUND` | None |
| Corrupted session file | File parsing | HTTP 400 with `FILE_PARSE_ERROR` | None |
| Session ID mismatch | File validation | HTTP 400 with `FILE_PARSE_ERROR` | None |
| Missing sessionId or cwd in file | File parsing | HTTP 400 with `FILE_PARSE_ERROR` | None |
| Working dir not accessible | Claude spawn | HTTP 500 with `WORKING_DIR_INVALID` | Clean up |
| Claude binary missing | Startup | **CRASH** orchestrator | Fix config |
| Claude spawn fails | Background worker | HTTP 500 with `CLAUDE_SPAWN_FAILED` | Clean up |
| Malformed JSON from Claude | Stdout parsing | Close all WebSockets, kill process | Session terminated |
| WebSocket to non-existent session | WS connection | Refuse connection | None |
| Client sends invalid JSON | WS message handler | Ignore message, log error | Continue |
| Write to dead Claude process | Stdin write | Close all WebSockets | Session terminated |
| Directory read error | Session listing | HTTP 500 with `DIRECTORY_READ_ERROR` | None |

## 10. Logging Specification

### 10.1 Log Levels and Events

#### INFO Level
- Server startup and configuration
- Session creation and termination
- Claude process lifecycle events
- WebSocket client connections and disconnections
- Session discovery results

#### DEBUG Level
- HTTP request/response details
- WebSocket message flow
- Write queue operations
- File parsing operations
- Directory scanning progress
- Claude stdin/stdout communication

#### ERROR Level
- Failed Claude process spawns
- Malformed JSON from Claude
- File parsing errors
- Directory access errors
- Process communication failures

### 10.2 Client Identification in Logs
Each WebSocket client should be logged with:
- IP address
- User-Agent header
- Connection timestamp
- Unique client ID (generated)

## 11. Performance Considerations

### 11.1 Resource Usage
- **Memory**: Unbounded write queue per session
- **Connections**: No limit on WebSocket connections per session
- **CPU**: JSON parsing for every message and file
- **File I/O**: Directory scanning for session listing and lookup

### 11.2 Concurrency Model
- **HTTP Server**: Thread/worker pool for requests
- **WebSocket Server**: Event loop or thread per connection
- **Background Workers**: Thread pool for session creation
- **Write Queue**: Synchronized per session
- **File Operations**: Should handle concurrent reads safely

## 12. Implementation Notes

### 12.1 Key Data Structures

Session object should contain:
- session_id: string
- working_dir: string
- process: Process reference
- clients: List of WebSocket clients
- write_queue: FIFO queue
- status: pending/ready/failed enumeration

WebSocket client object should contain:
- id: unique identifier
- connection: WebSocket connection reference
- ip_address: string
- user_agent: string
- connected_at: timestamp

### 12.2 Critical Invariants
1. Each session_id maps to at most one Claude process
2. Session IDs are globally unique across all projects
3. Write queue processes messages in FIFO order
4. Clients don't receive their own input echoed back
5. All clients receive Claude output
6. Background workers complete even if client disconnects
7. No buffering when no clients connected
8. Session ID in filename matches sessionId in file content

## 13. Security Considerations

### 13.1 Trust Model
- No authentication required
- Service not publicly exposed
- Full trust of all inputs
- No rate limiting

### 13.2 Input Validation
- Validate JSON structure
- Validate session file format
- No validation of working directory paths beyond existence
- No command injection prevention (trusted environment)

## 14. Example HTTP Flows

### 14.1 List All Sessions
```http
GET /api/v1/sessions HTTP/1.1

HTTP/1.1 200 OK
Content-Type: application/json

{
  "sessions": [
    {
      "session_id": "abc-123",
      "working_directory": "/home/user/project",
      "active": true
    },
    {
      "session_id": "def-456",
      "working_directory": "/home/user/other",
      "active": false
    }
  ]
}
```

### 14.2 Complete Session Creation
```http
POST /api/v1/sessions HTTP/1.1
Content-Type: application/json

{
  "session_id": "abc-123",
  "working_dir": "/home/user/project",
  "resume": false,
  "first_message": "{\"role\": \"user\", \"content\": \"Hello\"}"
}

HTTP/1.1 200 OK
Content-Type: application/json

{
  "session_id": "abc-123",
  "websocket_url": "/api/v1/sessions/abc-123/claude_ws",
  "approval_websocket_url": "/api/v1/sessions/abc-123/claude_approvals_ws"
}
```

### 14.3 Session Status Check
```http
GET /api/v1/sessions/abc-123 HTTP/1.1

HTTP/1.1 200 OK
Content-Type: application/json

{
  "session_id": "abc-123",
  "working_directory": "/home/user/project",
  "content": [
    {"type": "user", "message": {"role": "user", "content": "Hello"}},
    {"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Hi there!"}]}}
  ],
  "websocket_url": "/api/v1/sessions/abc-123/claude_ws",
  "approval_websocket_url": "/api/v1/sessions/abc-123/claude_approvals_ws"
}
```

