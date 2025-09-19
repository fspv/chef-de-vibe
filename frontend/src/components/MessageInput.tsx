import { useState, useEffect } from 'react';
import { v4 as uuidv4 } from 'uuid';
import type { PermissionMode } from '@anthropic-ai/claude-code/sdk';

interface MessageInputProps {
  onSendMessage: (message: string) => void;
  disabled: boolean;
  debugMode: boolean;
  isSessionActive?: boolean;
  isLoading?: boolean;
  initialValue?: string;
  currentMode?: PermissionMode;
  onSendMessages?: (messages: string[]) => void;
}

export function MessageInput({ 
  onSendMessage, 
  disabled, 
  debugMode, 
  isSessionActive = true, 
  isLoading = false, 
  initialValue = '',
  currentMode = 'default',
  onSendMessages
}: MessageInputProps) {
  const [input, setInput] = useState(initialValue);
  const [isMobile, setIsMobile] = useState(false);
  
  // Update input when initialValue changes (for restoring on error)
  useEffect(() => {
    if (initialValue && !input) {
      setInput(initialValue);
    }
  }, [initialValue, input]);
  
  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth <= 768 || 'ontouchstart' in window);
    };
    
    checkMobile();
    window.addEventListener('resize', checkMobile);
    return () => window.removeEventListener('resize', checkMobile);
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submitMessage();
  };

  const submitMessage = () => {
    if (input.trim() && !disabled && !isLoading) {
      if (debugMode) {
        // Raw JSON mode
        try {
          JSON.parse(input);
          onSendMessage(input);
          setInput('');
        } catch {
          alert('Invalid JSON format. Please check your input.');
        }
      } else {
        // Normal text mode - format as minimal Claude message
        const userMessage = {
          type: 'user',
          message: {
            role: 'user',
            content: input.trim()
          },
          parent_tool_use_id: null,
          uuid: uuidv4(),
          session_id: '' // This will be filled by the backend
        };
        
        // Always send control message first when session is active
        if (isSessionActive && onSendMessages) {
          const controlMessage = {
            request_id: Math.random().toString(36).substring(2, 15),
            type: "control_request",
            request: {
              subtype: "set_permission_mode",
              mode: currentMode
            }
          };
          
          // Send both messages
          onSendMessages([JSON.stringify(controlMessage), JSON.stringify(userMessage)]);
        } else {
          // Send only user message (for inactive sessions or if onSendMessages not provided)
          onSendMessage(JSON.stringify(userMessage));
        }
        
        // Always clear input after sending
        setInput('');
      }
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submitMessage();
    }
  };

  return (
    <form onSubmit={handleSubmit} className="message-input">
      <div className="input-area">
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={
            disabled 
              ? "Connecting..." 
              : isLoading
                ? "Resuming session..."
                : debugMode
                  ? "Enter raw JSON message (e.g., {\"role\": \"user\", \"content\": \"Hello\"})"
                  : !isSessionActive
                    ? isMobile
                      ? "Type to fork this session..."
                      : "Type to fork this session... (Ctrl/Cmd+Enter to send)"
                    : isMobile
                      ? "Type your message..."
                      : "Type your message... (Ctrl/Cmd+Enter to send)"
          }
          disabled={disabled || isLoading}
          rows={3}
        />
        <button type="submit" className="send-button" disabled={disabled || !input.trim() || isLoading}>
          {isLoading ? (
            // Loading spinner
            <svg width="34" height="34" viewBox="0 0 24 24" className="spinner">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" fill="none" strokeDasharray="31.4" strokeDashoffset="0">
                <animateTransform attributeName="transform" type="rotate" from="0 12 12" to="360 12 12" dur="1s" repeatCount="indefinite"/>
              </circle>
            </svg>
          ) : !isSessionActive ? (
            // Simple git fork icon for inactive sessions
            <svg width="34" height="34" viewBox="0 0 16 16" fill="currentColor">
              <path d="M5 3.25a.75.75 0 11-1.5 0 .75.75 0 011.5 0zm0 2.122a2.25 2.25 0 10-1.5 0v.878A2.25 2.25 0 005.75 8.5h1.5v2.128a2.251 2.251 0 101.5 0V8.5h1.5a2.25 2.25 0 002.25-2.25v-.878a2.25 2.25 0 10-1.5 0v.878a.75.75 0 01-.75.75h-4.5A.75.75 0 015.5 6.25v-.878zm3.75 7.378a.75.75 0 11-1.5 0 .75.75 0 011.5 0zm3-8.75a.75.75 0 100-1.5.75.75 0 000 1.5z"/>
            </svg>
          ) : (
            // Send arrow icon for active sessions  
            <svg width="34" height="34" viewBox="0 0 24 24" fill="currentColor" style={{ marginLeft: '2px' }}>
              <path d="M2 21l21-9L2 3v7l15 2-15 2v7z"/>
            </svg>
          )}
        </button>
      </div>
    </form>
  );
}