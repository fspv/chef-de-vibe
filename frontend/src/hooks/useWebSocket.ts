import { useState, useEffect, useCallback, useRef } from 'react';

export interface WebSocketMessage {
  data: unknown;
  timestamp: number;
}

export function useWebSocket(url: string | null) {
  const [isConnected, setIsConnected] = useState(false);
  const [messages, setMessages] = useState<WebSocketMessage[]>([]);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  const connect = useCallback(() => {
    if (!url || wsRef.current?.readyState === WebSocket.CONNECTING) return;

    try {
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        setIsConnected(true);
        setError(null);
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          setMessages(prev => [...prev, { data, timestamp: Date.now() }]);
        } catch (err) {
          console.error('Failed to parse WebSocket message:', err);
        }
      };

      ws.onclose = () => {
        setIsConnected(false);
      };

      ws.onerror = () => {
        setError('WebSocket connection error');
        setIsConnected(false);
      };
    } catch {
      setError('Failed to create WebSocket connection');
    }
  }, [url]);

  const disconnect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setIsConnected(false);
  }, []);

  const sendMessage = useCallback((message: string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(message);
    }
  }, []);

  const clearMessages = useCallback(() => {
    setMessages([]);
  }, []);

  const addMessage = useCallback((data: unknown) => {
    setMessages(prev => [...prev, { data, timestamp: Date.now() }]);
  }, []);

  useEffect(() => {
    // Clear messages when URL changes to prevent old session messages from showing
    setMessages([]);
    setError(null);
    
    if (url) {
      connect();
    } else {
      disconnect();
    }

    return () => {
      disconnect();
    };
  }, [url, connect, disconnect]);

  return {
    isConnected,
    messages,
    error,
    sendMessage,
    clearMessages,
    addMessage,
    reconnect: connect,
  };
}