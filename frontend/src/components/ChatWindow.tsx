import { useState, useCallback, useEffect, useRef } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { useLocation } from 'react-router-dom';
import { useSessionDetails } from '../hooks/useApi';
import { useWebSocket } from '../hooks/useWebSocket';
import { useApprovalWebSocket } from '../hooks/useApprovalWebSocket';
import { MessageList, type MessageListRef } from './MessageList';
import { MessageInput } from './MessageInput';
import { SessionStatusIndicator } from './SessionStatusIndicator';
import { api } from '../services/api';
import type { CreateSessionRequest, CreateSessionResponse } from '../types/api';
import type { PermissionUpdate } from '@anthropic-ai/claude-code/sdk';
import type { ToolInputSchemas } from '@anthropic-ai/claude-code/sdk-tools';

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
  const [autoScrollPaused, setAutoScrollPaused] = useState(false);
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



  const handleAutoScrollStateChange = useCallback((_isAtBottom: boolean, autoScrollPaused: boolean) => {
    setAutoScrollPaused(autoScrollPaused);
  }, []);

  const handleToggleAutoScroll = useCallback(() => {
    messageListRef.current?.toggleAutoScroll();
  }, []);

  if (!sessionId) {
    return (
      <div className="chat-window">
        {sidebarCollapsed && (
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
            autoScrollPaused={autoScrollPaused}
            onToggleAutoScroll={handleToggleAutoScroll}
          />
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
          autoScrollPaused={autoScrollPaused}
          onToggleAutoScroll={handleToggleAutoScroll}
        />
      )}

      <div className="chat-content">
        <MessageList 
          ref={messageListRef}
          sessionMessages={sessionDetails.content} 
          webSocketMessages={[...webSocketMessages, ...approvalWs.approvalMessages]}
          debugMode={debugMode}
          onAutoScrollStateChange={handleAutoScrollStateChange}
          onApprove={handleApprove}
          onDeny={handleDeny}
        />
      </div>

      <div className="chat-input">
        <MessageInput 
          onSendMessage={handleSendMessage}
          disabled={createLoading || (isActive && !isConnected)}
          debugMode={debugMode}
        />
      </div>
      
    </div>
  );
}
