# Chef de Vibe Frontend

A React TypeScript frontend for Chef de Vibe Service.

## Session Creation and Resumption

This document describes how the frontend handles session creation and resumption with Chef de Vibe service.

### Key Principle

Sessions are only created when the user sends their first message. The `first_message` field is required in all session creation requests (both new and resume).

### New Session Creation

**Initial State:**
- User is on the root URL (`/`) or clicks "New Chat"
- No session exists yet
- Chat interface shows an empty input field

**When User Sends First Message:**

1. **Generate Session ID**: Frontend creates a UUID for the new session

2. **Create Session with Message**:
   ```javascript
   POST /api/v1/sessions
   {
     "session_id": "generated-uuid",
     "working_dir": "/tmp",
     "resume": false,
     "first_message": "{\"role\": \"user\", \"content\": \"Hello Claude\"}"
   }
   ```
   Note: `first_message` contains the raw JSON string that the user typed in the input field

3. **Get WebSocket URLs from Response**:
   ```javascript
   {
     "session_id": "generated-uuid",
     "websocket_url": "/api/v1/sessions/generated-uuid/claude_ws",
     "approval_websocket_url": "/api/v1/sessions/generated-uuid/claude_approvals_ws"
   }
   ```

4. **Connect Both WebSockets Immediately**:
   ```javascript
   // Connect to chat WebSocket and start receiving messages
   const chatWs = new WebSocket(`ws://localhost:3000${response.websocket_url}`);
   chatWs.onmessage = handleMessage; // Capture all messages from this point
   
   // Connect to approval WebSocket for tool permission requests
   const approvalWs = new WebSocket(`ws://localhost:3000${response.approval_websocket_url}`);
   approvalWs.onmessage = handleApprovalRequest;
   ```

5. **Navigate to Session Page**:
   - Navigate to `/session/${session_id}`
   - The page will call GET `/api/v1/sessions/${session_id}` to load history
   - WebSocket is already capturing new messages

### Session Resume

**Initial State:**
- User navigates to an inactive session URL (e.g., `/session/old-session-id`)
- Backend returns session details without `websocket_url` field
- Chat shows previous conversation history
- Input field is enabled (no "Resume" button)

**When User Sends Message:**

1. **Resume Session with Message**:
   ```javascript
   POST /api/v1/sessions
   {
     "session_id": "old-session-id",
     "working_dir": "/home/user/project",
     "resume": true,
     "first_message": "{\"role\": \"user\", \"content\": \"Continue our discussion\"}"
   }
   ```
   Note: `first_message` is the raw JSON the user typed

2. **Get NEW Session ID and WebSocket URLs**:
   ```javascript
   {
     "session_id": "new-session-id",  // Different from request!
     "websocket_url": "/api/v1/sessions/new-session-id/claude_ws",
     "approval_websocket_url": "/api/v1/sessions/new-session-id/claude_approvals_ws"
   }
   ```

3. **Connect Both WebSockets Immediately**:
   ```javascript
   const chatWs = new WebSocket(`ws://localhost:3000${response.websocket_url}`);
   chatWs.onmessage = handleMessage; // Start capturing messages
   
   const approvalWs = new WebSocket(`ws://localhost:3000${response.approval_websocket_url}`);
   approvalWs.onmessage = handleApprovalRequest;
   ```

4. **Navigate to New Session**:
   - Navigate to `/session/${newSessionId}`
   - The page will call GET `/api/v1/sessions/${newSessionId}` to load history

### Message Flow Guarantee

The critical timing ensures no message loss:

```
Timeline:
1. POST /api/v1/sessions completes → Claude starts processing
2. Connect WebSocket → Start receiving Claude's responses
3. Navigate to new URL → Page loads
4. GET /api/v1/sessions/{id} → Fetch conversation history
5. Merge WebSocket messages with history → Some duplicates possible
```

**Why this works:**
- WebSocket captures messages that arrive between session creation and history fetch
- GET /api/v1/sessions/{id} provides the complete history up to that point
- Duplicates can occur when messages appear in both WebSocket stream and history
- **No messages are lost** because WebSocket is connected before navigation

### Message Input

Users input **raw JSON** directly into the text field:
```json
{"role": "user", "content": "Hello Claude"}
```

This raw JSON string is passed directly as the `first_message` parameter without any parsing or modification.

### Critical Implementation Details

1. **No Message Parsing**: User inputs raw JSON, frontend passes it as-is
2. **WebSocket Before Navigation**: Connect immediately after POST response
3. **Handle Duplicates**: Same message may appear from both WebSocket and GET history
4. **Session ID Changes on Resume**: Always use the response session_id for navigation

### Flow Summary

```
New Session:
1. User types raw JSON message
2. POST /api/v1/sessions (with first_message as raw JSON string)
3. Connect both WebSockets using response.websocket_url and response.approval_websocket_url
4. Navigate to /session/{id}
5. Page loads history via GET /api/v1/sessions/{id}
6. Merge WebSocket messages with history (handle duplicates)

Resume Session:
1. User types raw JSON in inactive session
2. POST /api/v1/sessions (resume=true, first_message as raw JSON)
3. Connect both WebSockets to NEW session URLs from response
4. Navigate to /session/{new-id}
5. Page loads history (includes old + new messages)
6. Continue receiving via both WebSockets
```

## Tool Approval System

The frontend includes a tool approval system that allows users to approve or deny Claude's requests to use specific tools (Read, Write, Bash, etc.).

### Architecture Overview

**Dual WebSocket Connection:**
- Main WebSocket: `/api/v1/sessions/{session_id}/claude_ws` - Regular Claude conversation
- Approval WebSocket: `/api/v1/sessions/{session_id}/claude_approvals_ws` - Tool permission requests

### Key Components

1. **ApprovalDialog Component**:
   - Modal dialog that displays tool usage requests
   - Shows tool name, input parameters (JSON formatted)
   - Provides Approve/Deny buttons
   - Handles permission rule suggestions

2. **useApprovalWebSocket Hook**:
   - Manages connection to approval WebSocket endpoint
   - Handles pending approvals on connection
   - Sends approval decisions back to backend

3. **ApprovalManager Service**:
   - Coordinates between approval WebSocket and UI components
   - Manages approval request queue and state
   - Handles approval response formatting

### User Flow

1. **Connection Setup**:
   ```javascript
   // Get URLs from session creation/resume response
   const sessionResponse = await postSession(sessionData);
   const { websocket_url, approval_websocket_url } = sessionResponse;
   
   // Connect to both WebSockets when session becomes active
   const chatWs = useWebSocket(websocket_url);
   const approvalWs = useApprovalWebSocket(approval_websocket_url);
   ```

2. **Approval Request Received**:
   - Backend sends approval request via approval WebSocket (both pending and new requests use the same format)
   - Frontend parses the raw Claude request to extract tool details
   - Frontend displays ApprovalDialog with tool details
   - User reviews tool name, input parameters, and suggestions

3. **User Decision**:
   - User clicks Approve or Deny
   - Frontend sends approval response using the backend's wrapped format:
   ```javascript
   // For approval:
   {
     "id": "uuid-1234",
     "response": {
       // Raw Claude response format - passed through by backend
       "behavior": "allow",
       "updatedInput": {...}, // Required, defaults to {} if not provided
       "updatedPermissions": [...] // Optional permission rule updates
     }
   }
   
   // For denial:
   {
     "id": "uuid-1234",
     "response": {
       // Raw Claude denial format - passed through by backend
       "behavior": "deny",
       "message": "Tool usage denied by user"
     }
   }
   ```

4. **Claude Continuation**:
   - Backend forwards decision to Claude
   - Claude continues execution based on approval
   - Regular conversation continues via main WebSocket

### Message Types

**Incoming (Server → Client):**

All approval requests (both pending and new) use the same simplified format:

```typescript
interface ApprovalRequestMessage {
  id: string;
  request: {
    // Raw Claude request.request content - not parsed by backend
    subtype: "can_use_tool";
    tool_name: string;
    input: Record<string, unknown>;
    permission_suggestions?: Array<Record<string, unknown>>;
  };
  created_at: string;
}
```

Note: The `request` field contains the inner `request` object from Claude's original control_request message, not the full control_request wrapper.

**Outgoing (Client → Server):**
```typescript
interface ApprovalResponseMessage {
  id: string; // Matches the id from the incoming request
  response: {
    // Raw Claude response format - backend passes through to Claude
    behavior: 'allow';
    updatedInput: Record<string, unknown>;
    updatedPermissions?: PermissionUpdate[];
  } | {
    behavior: 'deny';
    message: string;
    interrupt?: boolean;
  };
}
```

Note: The backend automatically handles setting the correct `request_id` from the original Claude request when forwarding the response to Claude.

### Implementation Details

**Persistent Requests:**
- Approval requests persist until explicitly resolved
- Reconnecting to approval WebSocket replays all pending requests
- No timeouts - users can approve requests hours/days later

**Multiple Clients:**
- Multiple browser tabs see the same approval requests
- Approving in one tab resolves the request for all tabs
- Real-time synchronization across client instances

**Backend Pass-Through Behavior:**
- Backend does not parse or validate the contents of `request` or `response` fields
- Backend only validates JSON structure and presence of required `id` field
- All Claude-specific message parsing is handled by frontend
- Backend acts as a simple proxy with request ID management
- All approval requests (pending and new) are sent as individual messages with the same format

**Error Handling:**
- Graceful degradation if approval WebSocket fails
- Retry connection logic with exponential backoff
- Clear user feedback for connection status
- Frontend must handle raw Claude request format parsing

## Development

```bash
cd frontend
npm install
npm run dev
```

## Environment Variables

```bash
VITE_API_BASE_URL=http://localhost:3000
VITE_WS_BASE_URL=ws://localhost:3000
```