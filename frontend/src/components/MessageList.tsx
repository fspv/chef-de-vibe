import { useEffect, useRef, useState, forwardRef, useImperativeHandle } from 'react';
import type { Message } from '../types/api';
import type { WebSocketMessage } from '../hooks/useWebSocket';
import { MessageParser } from './MessageParser';
import type { PermissionUpdate } from '@anthropic-ai/claude-code/sdk';
import type { ToolInputSchemas } from '@anthropic-ai/claude-code/sdk-tools';

interface MessageListProps {
  sessionMessages: Message[];
  webSocketMessages: WebSocketMessage[];
  debugMode: boolean;
  onAutoScrollStateChange?: (isAtBottom: boolean, autoScrollPaused: boolean) => void;
  onApprove?: (requestId: string, input: ToolInputSchemas, permissionUpdates?: PermissionUpdate[]) => void;
  onDeny?: (requestId: string) => void;
}

export interface MessageListRef {
  toggleAutoScroll: () => void;
}

export const MessageList = forwardRef<MessageListRef, MessageListProps>(({ sessionMessages, webSocketMessages, debugMode, onAutoScrollStateChange, onApprove, onDeny }, ref) => {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [hasInitiallyLoaded, setHasInitiallyLoaded] = useState(false);
  const [autoScrollPaused, setAutoScrollPaused] = useState(false);

  const checkIfAtBottom = () => {
    if (!messageListRef.current) return true;
    const { scrollTop, scrollHeight, clientHeight } = messageListRef.current;
    return scrollHeight - scrollTop - clientHeight < 50;
  };

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  const handleScrollToBottom = () => {
    scrollToBottom();
    setAutoScrollPaused(false);
  };

  const toggleAutoScroll = () => {
    setAutoScrollPaused(!autoScrollPaused);
  };

  // Expose functions to parent component
  useImperativeHandle(ref, () => ({
    toggleAutoScroll
  }));

  useEffect(() => {
    const handleScroll = () => {
      setIsAtBottom(checkIfAtBottom());
    };

    const container = messageListRef.current;
    if (container) {
      container.addEventListener('scroll', handleScroll);
      return () => container.removeEventListener('scroll', handleScroll);
    }
  }, []);

  useEffect(() => {
    if (!hasInitiallyLoaded && sessionMessages.length > 0) {
      scrollToBottom();
      setHasInitiallyLoaded(true);
    }
  }, [sessionMessages.length, hasInitiallyLoaded]);

  useEffect(() => {
    if (isAtBottom && !autoScrollPaused) {
      scrollToBottom();
    }
  }, [sessionMessages.length, webSocketMessages.length, isAtBottom, autoScrollPaused]);

  // Notify parent of state changes
  useEffect(() => {
    onAutoScrollStateChange?.(isAtBottom, autoScrollPaused);
  }, [isAtBottom, autoScrollPaused, onAutoScrollStateChange]);

  return (
    <div className="message-list" ref={messageListRef}>
      
      {sessionMessages.length === 0 && webSocketMessages.length === 0 ? (
        <div className="empty-messages">
          <p>No messages yet. Start a conversation!</p>
        </div>
      ) : (
        <div className="messages-container">
          {sessionMessages.map((message, index) => (
            <MessageParser
              key={`session-${index}`}
              data={message}
              showRawJson={debugMode}
              messageSource="session"
              onApprove={onApprove}
              onDeny={onDeny}
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
            />
          ))}
        </div>
      )}
      <div ref={messagesEndRef} />
      
      {/* Scroll to Bottom Button - only show when not at bottom */}
      {!isAtBottom && (
        <button 
          className="scroll-control-btn scroll-to-bottom-btn"
          onClick={handleScrollToBottom}
          title="Scroll to bottom"
          style={{
            position: 'fixed',
            bottom: '100px',
            right: '20px',
            zIndex: 1000
          }}
        >
          ⬇
        </button>
      )}
    </div>
  );
});
