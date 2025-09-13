import { useEffect, useRef } from 'react';
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

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(() => {
    scrollToBottom();
  }, [sessionMessages.length, webSocketMessages.length]);

  return (
    <div className="message-list">
      
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