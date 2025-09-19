import { useState, useCallback, useEffect, useRef, lazy, Suspense } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { useLocation } from 'react-router-dom';
import { useSessionDetails } from '../hooks/useApi';
import { useWebSocket } from '../hooks/useWebSocket';
import { useApprovalWebSocket } from '../hooks/useApprovalWebSocket';
import { MessageList, type MessageListRef } from './MessageList';
import { MessageInput } from './MessageInput';
// Lazy load the SessionStatusIndicator to prevent blocking
const SessionStatusIndicator = lazy(() => 
  import('./SessionStatusIndicator').then(module => ({
    default: module.SessionStatusIndicator
  }))
);
import { api } from '../services/api';
import type { CreateSessionRequest, CreateSessionResponse } from '../types/api';
import type { PermissionUpdate, PermissionMode } from '@anthropic-ai/claude-code/sdk';
import type { ToolInputSchemas } from '@anthropic-ai/claude-code/sdk-tools';
import { isSDKControlResponseMessage } from '../types/claude-messages';

interface ChatWindowProps {
  sessionId: string | null;
  onCreateSession: (request: CreateSessionRequest) => Promise<CreateSessionResponse | null>;
  createLoading: boolean;
  navigate: (path: string, options?: { state?: unknown }) => void;
  sidebarCollapsed: boolean;
  onNewChat: () => void;
}

export function ChatWindow({ sessionId, onCreateSession, createLoading, navigate, sidebarCollapsed, onNewChat }: ChatWindowProps) {
  const location = useLocation();
  const { sessionDetails, loading, error } = useSessionDetails(sessionId);
  const [debugMode, setDebugMode] = useState(false);
  const [currentMode, setCurrentMode] = useState<PermissionMode>('default');
  const messageListRef = useRef<MessageListRef>(null);
  
  // Helper function to ensure directory path starts with /
  const ensureAbsolutePath = (path: string | undefined | null): string => {
    if (!path) throw new Error('Working directory path is required but not provided');
    const cleanPath = path.trim();
    return cleanPath.startsWith('/') ? cleanPath : '/' + cleanPath;
  };
  
  // Get selected directory from navigation state (for new chats)
  const selectedDirectory = location.state?.selectedDirectory;
  
  const webSocketUrl = sessionDetails?.websocket_url 
    ? api.buildWebSocketUrl(sessionDetails.websocket_url)
    : null;
  
  const approvalWebSocketUrl = sessionDetails?.approval_websocket_url
    ? api.buildWebSocketUrl(sessionDetails.approval_websocket_url)
    : null;
  
  const { isConnected, messages: webSocketMessages, sendMessage, addMessage } = useWebSocket(webSocketUrl);
  const approvalWs = useApprovalWebSocket(approvalWebSocketUrl);
  const [pendingWebSocket, setPendingWebSocket] = useState<WebSocket | null>(null);
  const [pendingApprovalWebSocket, setPendingApprovalWebSocket] = useState<WebSocket | null>(null);

  // Approval handlers
  const handleApprove = useCallback((requestId: string, input: ToolInputSchemas, permissionUpdates: PermissionUpdate[] = []) => {
    // Send approval response through the approval websocket
    approvalWs.sendApprovalResponse({
      id: requestId,
      response: {
        behavior: 'allow',
        updatedInput: input as Record<string, unknown>,
        updatedPermissions: permissionUpdates
      }
    });
  }, [approvalWs]);

  const handleDeny = useCallback((requestId: string) => {
    // Send deny response through the approval websocket
    approvalWs.sendApprovalResponse({
      id: requestId,
      response: {
        behavior: 'deny',
        message: 'User denied the request'
      }
    });
  }, [approvalWs]);

  const handleSendMessage = useCallback(async (message: string) => {
    // Determine session state
    if (!sessionId) {
      // New session case
      const newSessionId = uuidv4();
      const request: CreateSessionRequest = {
        session_id: newSessionId,
        working_dir: ensureAbsolutePath(selectedDirectory),
        resume: false,
        bootstrap_messages: [message]
      };

      const response = await onCreateSession(request);
      if (response) {
        // Connect both WebSockets before navigation
        const wsUrl = api.buildWebSocketUrl(response.websocket_url);
        const approvalWsUrl = api.buildWebSocketUrl(response.approval_websocket_url);
        
        const ws = new WebSocket(wsUrl);
        const approvalWs = new WebSocket(approvalWsUrl);
        
        setPendingWebSocket(ws);
        setPendingApprovalWebSocket(approvalWs);
        
        // Wait briefly to ensure backend has fully processed the session before navigation
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        // Navigate to new session
        navigate(`/session/${response.session_id}`);
      }
    } else if (sessionDetails && !sessionDetails.websocket_url) {
      // Inactive session - need to resume  
      if (!sessionDetails.working_directory) {
        throw new Error('Cannot resume session: working directory not available');
      }
      const request: CreateSessionRequest = {
        session_id: sessionId,
        working_dir: sessionDetails.working_directory,
        resume: true,
        bootstrap_messages: [message]
      };

      const response = await onCreateSession(request);
      if (response) {
        // Connect both WebSockets to new session before navigation
        const wsUrl = api.buildWebSocketUrl(response.websocket_url);
        const approvalWsUrl = api.buildWebSocketUrl(response.approval_websocket_url);
        
        const ws = new WebSocket(wsUrl);
        const approvalWs = new WebSocket(approvalWsUrl);
        
        setPendingWebSocket(ws);
        setPendingApprovalWebSocket(approvalWs);
        
        // Wait briefly to ensure backend has fully processed the session before navigation
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        // Navigate to new session (ID will be different from request)
        navigate(`/session/${response.session_id}`);
      }
    } else {
      // Active session - send via WebSocket
      // Add the sent message to the display immediately
      if (debugMode) {
        // In debug mode, add raw JSON
        addMessage(JSON.parse(message));
      } else {
        // In normal mode, parse the formatted message
        const parsedMessage = JSON.parse(message);
        addMessage(parsedMessage);
      }
      sendMessage(message);
    }
  }, [sessionId, sessionDetails, onCreateSession, navigate, sendMessage, addMessage, debugMode, selectedDirectory]);

  // Clean up pending WebSockets on unmount
  useEffect(() => {
    return () => {
      if (pendingWebSocket && pendingWebSocket.readyState === WebSocket.OPEN) {
        pendingWebSocket.close();
      }
      if (pendingApprovalWebSocket && pendingApprovalWebSocket.readyState === WebSocket.OPEN) {
        pendingApprovalWebSocket.close();
      }
    };
  }, [pendingWebSocket, pendingApprovalWebSocket]);




  const handleModeChange = useCallback((newMode: PermissionMode) => {
    setCurrentMode(newMode);
    
    // Send control request to change permission mode
    if (sessionDetails?.websocket_url && isConnected) {
      const controlRequest = {
        request_id: Math.random().toString(36).substring(2, 15),
        type: "control_request",
        request: {
          subtype: "set_permission_mode",
          mode: newMode
        }
      };
      sendMessage(JSON.stringify(controlRequest));
    }
  }, [sessionDetails, isConnected, sendMessage]);

  // Listen for control response messages to update the mode
  useEffect(() => {
    if (webSocketMessages.length > 0) {
      const latestMessage = webSocketMessages[webSocketMessages.length - 1];
      if (isSDKControlResponseMessage(latestMessage.data)) {
        const controlResponse = latestMessage.data;
        if (controlResponse.response.subtype === 'success' && 
            controlResponse.response.response?.mode) {
          setCurrentMode(controlResponse.response.response.mode as PermissionMode);
        }
      }
    }
  }, [webSocketMessages]);

  if (!sessionId) {
    return (
      <div className="chat-window">
        {sidebarCollapsed && (
          <Suspense fallback={null}>
            <SessionStatusIndicator
              isActive={false}
              isMainConnected={false}
              isApprovalConnected={false}
              hasApprovalRequests={false}
              approvalRequestCount={0}
              sessionId="New Chat"
              workingDirectory={ensureAbsolutePath(selectedDirectory)}
              debugMode={debugMode}
              onDebugModeChange={setDebugMode}
              currentMode={currentMode}
              onModeChange={handleModeChange}
            />
          </Suspense>
        )}
        
        <div className="chat-content centered">
          <button 
            className="start-chat-button"
            onClick={onNewChat}
            disabled={createLoading}
          >
            Start New Chat
          </button>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="chat-window">
        <div className="loading-chat">
          <p>Loading session...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="chat-window">
        <div className="error-chat">
          <h3>Failed to resume chat session</h3>
          <p>{error}</p>
          <p className="error-suggestion">
            Please check the backend logs for more details. The session may have expired, 
            been corrupted, or the backend service may need to be restarted.
          </p>
        </div>
      </div>
    );
  }

  if (!sessionDetails) {
    return (
      <div className="chat-window">
        <div className="error-chat">
          <h3>Session not found</h3>
          <p>The requested session could not be found.</p>
          <p className="error-suggestion">
            The session may have been deleted or expired. Check the backend logs for more details 
            or try starting a new chat session.
          </p>
        </div>
      </div>
    );
  }

  const isActive = !!sessionDetails.websocket_url;

  return (
    <div className="chat-window">
      {sidebarCollapsed && (
        <Suspense fallback={null}>
          <SessionStatusIndicator
            isActive={isActive}
            isMainConnected={isConnected}
            isApprovalConnected={approvalWs.isConnected}
            hasApprovalRequests={approvalWs.pendingRequests.length > 0}
            approvalRequestCount={approvalWs.pendingRequests.length}
            sessionId={sessionDetails.session_id}
            workingDirectory={ensureAbsolutePath(sessionDetails.working_directory)}
            debugMode={debugMode}
            onDebugModeChange={setDebugMode}
            currentMode={currentMode}
            onModeChange={handleModeChange}
          />
        </Suspense>
      )}

      <div className="chat-content">
        <MessageList 
          ref={messageListRef}
          sessionMessages={sessionDetails.content} 
          webSocketMessages={[...webSocketMessages, ...approvalWs.approvalMessages]}
          debugMode={debugMode}
          onApprove={handleApprove}
          onDeny={handleDeny}
        />
        {!isActive && (
          <div className="session-finished-notice">
            <div className="notice-icon">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="currentColor">
                <path d="M6,2A3,3 0 0,1 9,5C9,6.28 8.19,7.38 7.06,7.81C7.15,8.27 7.39,8.83 8,9.63C9,10.92 11,12.83 12,14.17C13,12.83 15,10.92 16,9.63C16.61,8.83 16.85,8.27 16.94,7.81C15.81,7.38 15,6.28 15,5A3,3 0 0,1 18,2A3,3 0 0,1 21,5C21,6.32 20.14,7.45 18.95,7.85C18.87,8.37 18.64,9 18,9.83C17,11.17 15,13.08 14,14.38C13.39,15.17 13.15,15.73 13.06,16.19C14.19,16.62 15,17.72 15,19A3,3 0 0,1 12,22A3,3 0 0,1 9,19C9,17.72 9.81,16.62 10.94,16.19C10.85,15.73 10.61,15.17 10,14.38C9,13.08 7,11.17 6,9.83C5.36,9 5.13,8.37 5.05,7.85C3.86,7.45 3,6.32 3,5A3,3 0 0,1 6,2Z"/>
              </svg>
            </div>
            <div className="notice-content">
              <h4>Session Completed</h4>
              <p>This chat session has ended. Send a new message to fork this session and continue in a new one.</p>
            </div>
          </div>
        )}
      </div>

      <div className="chat-input">
        <MessageInput 
          onSendMessage={handleSendMessage}
          disabled={createLoading || (isActive && !isConnected)}
          debugMode={debugMode}
          isSessionActive={isActive}
        />
      </div>
      
    </div>
  );
}
