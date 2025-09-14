import { useEffect, useRef, useState } from 'react';
import type { Message } from '../types/api';
import type { WebSocketMessage } from '../hooks/useWebSocket';
import { MessageParser } from './MessageParser';

interface MessageListProps {
  sessionMessages: Message[];
  webSocketMessages: WebSocketMessage[];
  debugMode: boolean;
}

export function MessageList({ sessionMessages, webSocketMessages, debugMode }: MessageListProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [hasInitiallyLoaded, setHasInitiallyLoaded] = useState(false);

  const checkIfAtBottom = () => {
    if (!messageListRef.current) return true;
    const { scrollTop, scrollHeight, clientHeight } = messageListRef.current;
    return scrollHeight - scrollTop - clientHeight < 50;
  };

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

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
    if (isAtBottom) {
      scrollToBottom();
    }
  }, [sessionMessages.length, webSocketMessages.length, isAtBottom]);

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
            />
          ))}
          {webSocketMessages.map((wsMessage, index) => (
            <MessageParser
              key={`ws-${index}`}
              data={wsMessage.data}
              timestamp={wsMessage.timestamp}
              showRawJson={debugMode}
              messageSource="websocket"
            />
          ))}
        </div>
      )}
      <div ref={messagesEndRef} />
    </div>
  );
}