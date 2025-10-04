import { useState, useEffect, useRef } from 'react';
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
  onHeightChange?: (height: number) => void;
  onMessageSent?: (uuid: string) => void;
  isWaitingForEcho?: boolean;
}

export function MessageInput({ 
  onSendMessage, 
  disabled, 
  debugMode, 
  isSessionActive = true, 
  isLoading = false, 
  initialValue = '',
  currentMode = 'default',
  onSendMessages,
  onHeightChange,
  onMessageSent,
  isWaitingForEcho = false
}: MessageInputProps) {
  const [input, setInput] = useState(initialValue);
  const [isMobile, setIsMobile] = useState(false);
  const [textareaHeight, setTextareaHeight] = useState(120); // Default height in pixels
  const [isResizing, setIsResizing] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const sendTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const resizeHandleRef = useRef<HTMLDivElement>(null);
  const inputAreaRef = useRef<HTMLDivElement>(null);
  const animationFrameRef = useRef<number | null>(null);
  const currentHeightRef = useRef<number>(120); // Track current height for DOM manipulation
  
  // Update input when initialValue changes (for restoring on error)
  useEffect(() => {
    if (initialValue && !input) {
      setInput(initialValue);
    }
  }, [initialValue, input]);
  
  // Handle echo received
  useEffect(() => {
    if (!isWaitingForEcho && isSending) {
      // Echo received, clear the input and reset state
      setInput('');
      setIsSending(false);
      if (sendTimeoutRef.current) {
        clearTimeout(sendTimeoutRef.current);
        sendTimeoutRef.current = null;
      }
    }
  }, [isWaitingForEcho, isSending]);
  
  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (sendTimeoutRef.current) {
        clearTimeout(sendTimeoutRef.current);
      }
    };
  }, []);
  
  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth <= 768 || 'ontouchstart' in window);
    };
    
    checkMobile();
    window.addEventListener('resize', checkMobile);
    return () => window.removeEventListener('resize', checkMobile);
  }, []);

  // Notify parent about height changes
  useEffect(() => {
    if (onHeightChange) {
      // Total height = textarea height + resize handle (8px) + any padding
      // Adding some extra space for the input area wrapper
      onHeightChange(textareaHeight + 20);
    }
  }, [textareaHeight, onHeightChange]);

  // Handle resize drag (mouse and touch) with optimizations
  useEffect(() => {
    const updateHeight = (newHeight: number) => {
      // Apply height directly to DOM during resize to avoid React re-renders
      if (inputAreaRef.current && isResizing) {
        // Remove transition during active resizing
        inputAreaRef.current.style.transition = 'none';
        inputAreaRef.current.style.height = `${newHeight}px`;
        currentHeightRef.current = newHeight;
      }
    };

    const handleMove = (clientY: number) => {
      if (!isResizing || !textareaRef.current) return;
      
      const containerRect = textareaRef.current.parentElement?.getBoundingClientRect();
      if (!containerRect) return;
      
      const newHeight = containerRect.bottom - clientY;
      // Constrain between min and max heights
      const constrainedHeight = Math.max(60, Math.min(400, newHeight));
      
      // Cancel any pending animation frame
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      
      // Use requestAnimationFrame to throttle updates
      animationFrameRef.current = requestAnimationFrame(() => {
        updateHeight(constrainedHeight);
      });
    };

    const handleMouseMove = (e: MouseEvent) => {
      handleMove(e.clientY);
    };

    const handleTouchMove = (e: TouchEvent) => {
      if (e.touches.length === 1) {
        e.preventDefault(); // Prevent scrolling while resizing
        handleMove(e.touches[0].clientY);
      }
    };

    const handleEnd = () => {
      setIsResizing(false);
      document.body.style.userSelect = '';
      document.body.style.cursor = '';
      document.body.style.touchAction = ''; // Reset touch-action
      
      // Cancel any pending animation frame
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
      
      // Update React state only when resize ends
      if (inputAreaRef.current) {
        // Restore transition
        inputAreaRef.current.style.transition = '';
        setTextareaHeight(currentHeightRef.current);
      }
    };

    if (isResizing) {
      document.body.style.userSelect = 'none';
      document.body.style.cursor = 'ns-resize';
      document.body.style.touchAction = 'none'; // Prevent scrolling on touch
      
      // Mouse events
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleEnd);
      
      // Touch events
      document.addEventListener('touchmove', handleTouchMove, { passive: false });
      document.addEventListener('touchend', handleEnd);
      document.addEventListener('touchcancel', handleEnd);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleEnd);
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('touchend', handleEnd);
      document.removeEventListener('touchcancel', handleEnd);
      
      // Cleanup animation frame on unmount
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [isResizing]);

  const handleResizeStart = (e: React.MouseEvent | React.TouchEvent) => {
    e.preventDefault();
    setIsResizing(true);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submitMessage();
  };

  const submitMessage = () => {
    if (input.trim() && !disabled && !isLoading && !isSending) {
      const messageUuid = uuidv4();
      setIsSending(true);
      
      // Set a timeout to unlock after 5 seconds if no echo
      sendTimeoutRef.current = setTimeout(() => {
        setIsSending(false);
        setInput(''); // Clear input on timeout
      }, 5000);
      
      if (debugMode) {
        // Raw JSON mode
        try {
          JSON.parse(input);
          onSendMessage(input);
          onMessageSent?.(messageUuid);
          // Don't clear input here - wait for echo
        } catch {
          alert('Invalid JSON format. Please check your input.');
          setIsSending(false);
          if (sendTimeoutRef.current) {
            clearTimeout(sendTimeoutRef.current);
            sendTimeoutRef.current = null;
          }
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
          uuid: messageUuid,
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
        
        // Notify parent of sent message UUID
        onMessageSent?.(messageUuid);
        // Don't clear input here - wait for echo
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
      <div className="resize-handle" 
           ref={resizeHandleRef}
           onMouseDown={handleResizeStart}
           onTouchStart={handleResizeStart}
           title="Drag to resize">
        <div className="resize-handle-bar"></div>
      </div>
      <div className="input-area" ref={inputAreaRef} style={{ height: `${textareaHeight}px` }}>
        <textarea
          ref={textareaRef}
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
          disabled={disabled || isLoading || isSending}
          style={{ height: '100%' }}
        />
        <button type="submit" className="send-button" disabled={disabled || !input.trim() || isLoading || isSending}>
          {(isLoading || isSending) ? (
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