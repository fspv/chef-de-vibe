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

export function ChatWindow({ sessionId, onCreateSession, createLoading, navigate, onNewChat }: ChatWindowProps) {
  const location = useLocation();
  const { sessionDetails, loading, error } = useSessionDetails(sessionId);
  const [debugMode, setDebugMode] = useState(false);
  
  // Get initial mode from navigation state (when coming from new chat dialog)
  const initialModeFromNav = (location.state as { initialMode?: PermissionMode })?.initialMode;
  const [currentMode, setCurrentMode] = useState<PermissionMode>(initialModeFromNav || 'default');
  const [hasSetInitialMode, setHasSetInitialMode] = useState(false);
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
  const approvalWs = useApprovalWebSocket(approvalWebSocketUrl, sessionId);
  const [pendingWebSocket, setPendingWebSocket] = useState<WebSocket | null>(null);
  const [pendingApprovalWebSocket, setPendingApprovalWebSocket] = useState<WebSocket | null>(null);
  const [isResumingSession, setIsResumingSession] = useState(false);
  const [pendingMessage, setPendingMessage] = useState<string | null>(null);

  // Handle mode initialization when sessionId or navigation state changes
  useEffect(() => {
    if (initialModeFromNav && !hasSetInitialMode) {
      // We have an initial mode from navigation state (new chat was just created)
      setCurrentMode(initialModeFromNav);
      setHasSetInitialMode(true);
    } else if (!initialModeFromNav && sessionId) {
      // No initial mode from nav, reset to default for existing sessions
      setCurrentMode('default');
      setHasSetInitialMode(false);
    }
  }, [sessionId, initialModeFromNav, hasSetInitialMode]);

  // Approval handlers
  const handleApprove = useCallback(async (requestId: string, input: ToolInputSchemas, permissionUpdates: PermissionUpdate[] = []): Promise<void> => {
    // Send approval response through the approval websocket
    return approvalWs.sendApprovalResponse({
      id: requestId,
      response: {
        behavior: 'allow',
        updatedInput: input as Record<string, unknown>,
        updatedPermissions: permissionUpdates
      }
    });
  }, [approvalWs]);

  const handleDeny = useCallback(async (requestId: string): Promise<void> => {
    // Send deny response through the approval websocket
    return approvalWs.sendApprovalResponse({
      id: requestId,
      response: {
        behavior: 'deny',
        message: 'User denied the request'
      }
    });
  }, [approvalWs]);

  const handleSendMessage = useCallback(async (message: string, allMessages?: string[]) => {
    // Determine session state
    if (!sessionId) {
      // New session case
      setIsSessionLoading(true);
      setLoadingOperation('creating');
      setLoadingLogs([]);
      
      const newSessionId = uuidv4();
      addLog(`Generating new session ID: ${newSessionId}`, 'info');
      
      // Include control message in bootstrap messages
      const bootstrapMessages: string[] = [];
      
      // Add control message for mode
      const controlMessage = {
        request_id: Math.random().toString(36).substring(2, 15),
        type: "control_request",
        request: {
          subtype: "set_permission_mode",
          mode: currentMode
        }
      };
      bootstrapMessages.push(JSON.stringify(controlMessage));
      
      // Add the user message
      bootstrapMessages.push(message);
      
      const request: CreateSessionRequest = {
        session_id: newSessionId,
        working_dir: ensureAbsolutePath(selectedDirectory),
        resume: false,
        bootstrap_messages: bootstrapMessages
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
        // Navigate to new session with current mode
        navigate(`/session/${response.session_id}`, { state: { initialMode: currentMode } });
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
        // Include all messages (control + user) in bootstrap for fork
        const bootstrapMessages: string[] = allMessages || [];
        
        // If no allMessages provided (backward compatibility), create control message
        if (!allMessages || allMessages.length === 0) {
          const controlMessage = {
            request_id: Math.random().toString(36).substring(2, 15),
            type: "control_request",
            request: {
              subtype: "set_permission_mode",
              mode: currentMode
            }
          };
          bootstrapMessages.push(JSON.stringify(controlMessage));
          bootstrapMessages.push(message);
        }
        
        const request: CreateSessionRequest = {
          session_id: sessionId,
          working_dir: sessionDetails.working_directory,
          resume: true,
          bootstrap_messages: bootstrapMessages
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
      // Server will echo the message back, so we don't add it locally
      sendMessage(message);
    }
  }, [sessionId, sessionDetails, onCreateSession, navigate, sendMessage, addMessage, selectedDirectory, addLog, currentMode]);

  // Handle sending multiple messages (for control + user message)
  const handleSendMessages = useCallback(async (messages: string[]) => {
    // For active sessions, send all messages through WebSocket
    const sessionIsActive = sessionDetails && !!sessionDetails.websocket_url;
    if (sessionIsActive && isConnected) {
      messages.forEach(msg => sendMessage(msg));
    } else {
      // For inactive/new sessions, use the first user message for bootstrap
      const userMessage = messages.find(msg => {
        try {
          const parsed = JSON.parse(msg);
          return parsed.type === 'user';
        } catch {
          return false;
        }
      });
      
      if (userMessage) {
        await handleSendMessage(userMessage, messages);
      }
    }
  }, [sessionDetails, isConnected, sendMessage, handleSendMessage]);

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

  const handleInterrupt = useCallback(() => {
    // Send interrupt control request
    if (sessionDetails?.websocket_url && isConnected) {
      const interruptRequest = {
        request_id: Math.random().toString(36).substring(2, 15),
        type: "control_request",
        request: {
          subtype: "interrupt"
        }
      };
      sendMessage(JSON.stringify(interruptRequest));
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
  
  // Add session completed message when session becomes inactive
  useEffect(() => {
    if (sessionDetails && !sessionDetails.websocket_url && sessionId) {
      // Add a system message indicating session has completed
      const sessionCompletedMessage = {
        type: 'system',
        message: { 
          content: '**Session Completed**\n\nThis chat session has ended. Send a new message to fork this session and continue in a new one.' 
        },
        uuid: `session-completed-${sessionId}`,
        session_id: sessionId
      };
      
      // Check if we already have this message to avoid duplicates
      const hasCompletedMessage = webSocketMessages.some(msg => 
        (msg.data as { uuid?: string })?.uuid === sessionCompletedMessage.uuid
      );
      
      if (!hasCompletedMessage) {
        addMessage(sessionCompletedMessage);
      }
    }
  }, [sessionDetails, sessionId, webSocketMessages, addMessage]);
  
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
            onInterrupt={undefined}
          />
        </Suspense>
        
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
          onInterrupt={handleInterrupt}
        />
      </Suspense>

      <div className="chat-content">
        <MessageList 
          ref={messageListRef}
          sessionMessages={sessionDetails.content} 
          webSocketMessages={[
            ...webSocketMessages.filter(msg => {
              const data = msg.data as { type?: string; request?: { subtype?: string } };
              // Filter out control_request messages from main WebSocket as they come through approval WebSocket
              return data?.type !== 'control_request' || data?.request?.subtype !== 'can_use_tool';
            }), 
            ...(sessionId && approvalWebSocketUrl ? approvalWs.approvalMessages : [])
          ]}
          debugMode={debugMode}
          onApprove={handleApprove}
          onDeny={handleDeny}
          onModeChange={handleModeChange}
        />
      </div>

      <div className="chat-input">
        <MessageInput 
          onSendMessage={handleSendMessage}
          onSendMessages={handleSendMessages}
          disabled={createLoading || (isActive && !isConnected)}
          debugMode={debugMode}
          isSessionActive={isActive}
          isLoading={isResumingSession}
          initialValue={pendingMessage || ''}
          currentMode={currentMode}
        />
      </div>
      
    </div>
  );
}
