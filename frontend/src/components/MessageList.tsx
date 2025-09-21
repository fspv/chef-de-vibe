import { useEffect, useRef, useState, forwardRef, useImperativeHandle, useLayoutEffect } from 'react';
import type { Message } from '../types/api';
import type { WebSocketMessage } from '../hooks/useWebSocket';
import { MessageParser } from './MessageParser';
import type { PermissionUpdate, PermissionMode } from '@anthropic-ai/claude-code/sdk';
import type { ToolInputSchemas } from '@anthropic-ai/claude-code/sdk-tools';

interface MessageListProps {
  sessionMessages: Message[];
  webSocketMessages: WebSocketMessage[];
  debugMode: boolean;
  onApprove?: (requestId: string, input: ToolInputSchemas, permissionUpdates?: PermissionUpdate[]) => void;
  onDeny?: (requestId: string) => void;
  onModeChange?: (mode: PermissionMode) => void;
  inputHeight?: number;
}

export interface MessageListRef {
  toggleAutoScroll: () => void;
}

export const MessageList = forwardRef<MessageListRef, MessageListProps>(({ sessionMessages, webSocketMessages, debugMode, onApprove, onDeny, onModeChange, inputHeight = 140 }, ref) => {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [hasInitiallyScrolled, setHasInitiallyScrolled] = useState(false);
  const [isAutoScrolling, setIsAutoScrolling] = useState(false);
  const autoScrollTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const checkIfAtBottom = () => {
    if (!messageListRef.current) {
      return true;
    }
    const { scrollTop, scrollHeight, clientHeight } = messageListRef.current;
    const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
    const atBottom = distanceFromBottom < 50;
    return atBottom;
  };

  const scrollToBottom = (smooth = false) => {
    if (messageListRef.current) {
      const container = messageListRef.current;
      const targetScroll = container.scrollHeight;
      
      // Mark that we're auto-scrolling
      if (smooth) {
        setIsAutoScrolling(true);
        // Clear any existing timeout
        if (autoScrollTimeoutRef.current) {
          clearTimeout(autoScrollTimeoutRef.current);
        }
        // Set timeout to clear auto-scrolling flag after animation completes
        autoScrollTimeoutRef.current = setTimeout(() => {
          setIsAutoScrolling(false);
        }, 500); // Smooth scroll animation typically takes ~300-400ms
      }
      
      if (smooth) {
        container.scrollTo({
          top: targetScroll,
          behavior: 'smooth'
        });
      } else {
        container.scrollTop = targetScroll;
      }
    }
    
    // Also try scrollIntoView for instant scroll
    if (messagesEndRef.current && !smooth) {
      messagesEndRef.current.scrollIntoView({ behavior: 'auto', block: 'end' });
    }
  };

  const handleScrollToBottom = () => {
    scrollToBottom(true);
    setIsAtBottom(true);
  };

  const toggleAutoScroll = () => {
    // This function is kept for compatibility but doesn't do anything
    // since we removed the pause functionality
  };

  // Expose functions to parent component
  useImperativeHandle(ref, () => ({
    toggleAutoScroll
  }));

  // Monitor scroll position
  useEffect(() => {
    const handleScroll = () => {
      const atBottom = checkIfAtBottom();
      if (atBottom !== isAtBottom) {
        setIsAtBottom(atBottom);
      }
    };

    const container = messageListRef.current;
    if (container) {
      container.addEventListener('scroll', handleScroll, { passive: true });
      return () => {
        container.removeEventListener('scroll', handleScroll);
      };
    }
  }, [isAtBottom]);

  // Initial scroll when messages first appear
  useLayoutEffect(() => {
    const totalMessages = sessionMessages.length + webSocketMessages.length;
    
    if (!hasInitiallyScrolled && totalMessages > 0 && messageListRef.current) {
      // Force a layout recalculation
      void messageListRef.current.offsetHeight;
      
      // Try immediate scroll
      scrollToBottom(false);
      setHasInitiallyScrolled(true);
      
      // Also try after a delay as backup
      setTimeout(() => {
        scrollToBottom(false);
      }, 500);
    }
  }, [sessionMessages, webSocketMessages, hasInitiallyScrolled]);

  // Auto-scroll for new messages
  useEffect(() => {
    const totalMessages = sessionMessages.length + webSocketMessages.length;
    
    if (hasInitiallyScrolled && totalMessages > 0 && isAtBottom) {
      // Use requestAnimationFrame to ensure DOM is updated
      requestAnimationFrame(() => {
        scrollToBottom(true);
      });
    }
  }, [sessionMessages.length, webSocketMessages.length, isAtBottom, hasInitiallyScrolled]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (autoScrollTimeoutRef.current) {
        clearTimeout(autoScrollTimeoutRef.current);
      }
    };
  }, []);

  // Only show button if not at bottom AND we're not currently auto-scrolling
  const shouldShowScrollButton = !isAtBottom && !isAutoScrolling;

  return (
    <div className="message-list" ref={messageListRef} style={{ position: 'relative' }}>
      
      {sessionMessages.length === 0 && webSocketMessages.length === 0 ? (
        <div className="empty-messages">
          <p>No messages yet. Start a conversation!</p>
        </div>
      ) : (
        <>
          {sessionMessages.map((message, index) => (
            <MessageParser
              key={`session-${index}`}
              data={message}
              showRawJson={debugMode}
              messageSource="session"
              onApprove={onApprove}
              onDeny={onDeny}
              onModeChange={onModeChange}
            />
          ))}
          {webSocketMessages.map((wsMessage, index) => (
            <MessageParser
              key={`ws-${index}`}
              data={wsMessage.data}
              timestamp={wsMessage.timestamp}
              showRawJson={debugMode}
              messageSource="websocket"
              onApprove={onApprove}
              onDeny={onDeny}
              onModeChange={onModeChange}
            />
          ))}
        </>
      )}
      
      <div ref={messagesEndRef} style={{ height: '1px', width: '100%' }} />
      
      {/* Scroll to Bottom Button */}
      {shouldShowScrollButton && (
        <button 
          className="scroll-to-bottom-btn"
          onClick={handleScrollToBottom}
          title="Scroll to bottom"
          style={{
            position: 'fixed',
            bottom: `${inputHeight + 20}px`,
            right: '30px',
            zIndex: 1000
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
            <path d="M7 10l5 5 5-5z"/>
          </svg>
        </button>
      )}
    </div>
  );
});