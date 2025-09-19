import { useState, useCallback, useEffect, useRef, lazy, Suspense } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { useLocation } from 'react-router-dom';
import { useSessionDetails } from '../hooks/useApi';
import { useWebSocket } from '../hooks/useWebSocket';
import { useApprovalWebSocket } from '../hooks/useApprovalWebSocket';
import { MessageList, type MessageListRef } from './MessageList';
import { MessageInput } from './MessageInput';
import { LoadingScreen, type LogEntry } from './LoadingScreen';
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
  const [loadingLogs, setLoadingLogs] = useState<LogEntry[]>([]);
  const [loadingOperation, setLoadingOperation] = useState<'creating' | 'resuming' | 'loading'>('loading');
  const [isSessionLoading, setIsSessionLoading] = useState(false);
  
  // Helper function to ensure directory path starts with /
  const ensureAbsolutePath = (path: string | undefined | null): string => {
    if (!path) throw new Error('Working directory path is required but not provided');
    const cleanPath = path.trim();
    return cleanPath.startsWith('/') ? cleanPath : '/' + cleanPath;
  };
  
  // Get selected directory from navigation state (for new chats)
  const selectedDirectory = location.state?.selectedDirectory;
  
  const addLog = useCallback((message: string, type: 'info' | 'success' | 'error' | 'warning' = 'info') => {
    setLoadingLogs(prev => [...prev, { timestamp: Date.now(), message, type }]);
  }, []);
  
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
  const [isResumingSession, setIsResumingSession] = useState(false);
  const [pendingMessage, setPendingMessage] = useState<string | null>(null);

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
      setIsSessionLoading(true);
      setLoadingOperation('creating');
      setLoadingLogs([]);
      
      const newSessionId = uuidv4();
      addLog(`Generating new session ID: ${newSessionId}`, 'info');
      
      const request: CreateSessionRequest = {
        session_id: newSessionId,
        working_dir: ensureAbsolutePath(selectedDirectory),
        resume: false,
        bootstrap_messages: [message]
      };
      
      addLog(`Creating session with working directory: ${request.working_dir}`, 'info');
      addLog('Sending create session request to backend...', 'info');

      const response = await onCreateSession(request);
      if (response) {
        addLog(`Session created successfully with ID: ${response.session_id}`, 'success');
        
        // Connect both WebSockets before navigation
        const wsUrl = api.buildWebSocketUrl(response.websocket_url);
        const approvalWsUrl = api.buildWebSocketUrl(response.approval_websocket_url);
        
        addLog('Connecting to main WebSocket...', 'info');
        const ws = new WebSocket(wsUrl);
        
        ws.onopen = () => addLog('Main WebSocket connected', 'success');
        ws.onerror = () => addLog('Main WebSocket connection error', 'error');
        
        addLog('Connecting to approval WebSocket...', 'info');
        const approvalWs = new WebSocket(approvalWsUrl);
        
        approvalWs.onopen = () => addLog('Approval WebSocket connected', 'success');
        approvalWs.onerror = () => addLog('Approval WebSocket connection error', 'error');
        
        setPendingWebSocket(ws);
        setPendingApprovalWebSocket(approvalWs);
        
        addLog('Waiting for session initialization...', 'info');
        // Wait briefly to ensure backend has fully processed the session before navigation
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        addLog('Navigating to session...', 'success');
        // Navigate to new session
        navigate(`/session/${response.session_id}`);
      } else {
        addLog('Failed to create session', 'error');
        setIsSessionLoading(false);
      }
    } else if (sessionDetails && !sessionDetails.websocket_url) {
      // Inactive session - need to resume
      // Store the message in case we need to restore it on failure
      const parsedMessage = JSON.parse(message);
      const messageContent = parsedMessage.message?.content || '';
      setPendingMessage(messageContent);
      
      setIsResumingSession(true);
      setIsSessionLoading(true);
      setLoadingOperation('resuming');
      setLoadingLogs([]);
      
      addLog(`Resuming session: ${sessionId}`, 'info');
      
      if (!sessionDetails.working_directory) {
        addLog('Cannot resume session: working directory not available', 'error');
        setIsSessionLoading(false);
        setIsResumingSession(false);
        setPendingMessage(null);
        // Add error message to chat
        addMessage({
          type: 'system',
          message: { content: 'Error: Cannot resume session - working directory not available' },
          uuid: uuidv4(),
          session_id: sessionId
        });
        return;
      }
      
      try {
        const request: CreateSessionRequest = {
          session_id: sessionId,
          working_dir: sessionDetails.working_directory,
          resume: true,
          bootstrap_messages: [message]
        };
        
        addLog(`Resuming with working directory: ${request.working_dir}`, 'info');
        addLog('Sending resume session request to backend...', 'info');

        const response = await onCreateSession(request);
        if (response) {
          addLog(`Session resumed successfully with new ID: ${response.session_id}`, 'success');
          
          // Connect both WebSockets to new session before navigation
          const wsUrl = api.buildWebSocketUrl(response.websocket_url);
          const approvalWsUrl = api.buildWebSocketUrl(response.approval_websocket_url);
          
          addLog('Connecting to main WebSocket...', 'info');
          const ws = new WebSocket(wsUrl);
          
          ws.onopen = () => addLog('Main WebSocket connected', 'success');
          ws.onerror = () => addLog('Main WebSocket connection error', 'error');
          
          addLog('Connecting to approval WebSocket...', 'info');
          const approvalWs = new WebSocket(approvalWsUrl);
          
          approvalWs.onopen = () => addLog('Approval WebSocket connected', 'success');
          approvalWs.onerror = () => addLog('Approval WebSocket connection error', 'error');
          
          setPendingWebSocket(ws);
          setPendingApprovalWebSocket(approvalWs);
          
          addLog('Waiting for session initialization...', 'info');
          // Wait briefly to ensure backend has fully processed the session before navigation
          await new Promise(resolve => setTimeout(resolve, 2000));
          
          // Clear pending message on success before navigation
          setPendingMessage(null);
          setIsResumingSession(false);
          
          addLog('Navigating to resumed session...', 'success');
          // Navigate to new session (ID will be different from request)
          navigate(`/session/${response.session_id}`);
        } else {
          throw new Error('Failed to resume session - no response from backend');
        }
      } catch (error) {
        addLog('Failed to resume session', 'error');
        setIsSessionLoading(false);
        setIsResumingSession(false);
        // Don't clear pendingMessage here - we'll use it to restore the input
        // Add error message to chat
        const errorMessage = error instanceof Error ? error.message : 'Failed to resume session';
        addMessage({
          type: 'system',
          message: { content: `Error: ${errorMessage}. Please check the backend logs for more details.` },
          uuid: uuidv4(),
          session_id: sessionId
        });
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
  }, [sessionId, sessionDetails, onCreateSession, navigate, sendMessage, addMessage, debugMode, selectedDirectory, addLog]);

  // Clear pendingMessage after it's been set to input
  useEffect(() => {
    if (pendingMessage && !isResumingSession) {
      // Give the MessageInput component time to use the value
      const timer = setTimeout(() => {
        setPendingMessage(null);
      }, 100);
      return () => clearTimeout(timer);
    }
  }, [pendingMessage, isResumingSession]);

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
  
  // Monitor WebSocket connections for logging
  useEffect(() => {
    if (loading && sessionId) {
      setIsSessionLoading(true);
      setLoadingOperation('loading');
      setLoadingLogs([]);
      addLog(`Getting session details for: ${sessionId}`, 'info');
    }
  }, [loading, sessionId, addLog]);
  
  useEffect(() => {
    if (sessionDetails && isSessionLoading) {
      addLog('Session details retrieved', 'success');
      
      if (sessionDetails.websocket_url) {
        addLog('Connecting to main WebSocket...', 'info');
      } else {
        addLog('Session is inactive, send a message to resume', 'warning');
        setIsSessionLoading(false);
      }
    }
  }, [sessionDetails, isSessionLoading, addLog]);
  
  useEffect(() => {
    if (isConnected && isSessionLoading) {
      addLog('Main WebSocket connected successfully', 'success');
    }
  }, [isConnected, isSessionLoading, addLog]);
  
  useEffect(() => {
    if (approvalWs.isConnected && isSessionLoading) {
      addLog('Approval WebSocket connected successfully', 'success');
      addLog('Session ready!', 'success');
      setTimeout(() => setIsSessionLoading(false), 1000);
    }
  }, [approvalWs.isConnected, isSessionLoading, addLog]);

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

  if (loading || isSessionLoading) {
    return (
      <LoadingScreen 
        sessionId={sessionId}
        operation={loadingOperation}
        logs={loadingLogs}
      />
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
          isLoading={isResumingSession}
          initialValue={pendingMessage || ''}
        />
      </div>
      
    </div>
  );
}
