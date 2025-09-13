import { useState, useEffect, useCallback, useRef } from 'react';
import type { ApprovalRequest, ApprovalResponseMessage } from '../types/api';
import type { PermissionUpdate } from '@anthropic-ai/claude-code/sdk';
import { ApprovalRequestMessageSchema } from '../types/approvalSchemas';

export interface ApprovalWebSocketHookReturn {
  isConnected: boolean;
  pendingRequests: ApprovalRequest[];
  error: string | null;
  sendApprovalResponse: (response: ApprovalResponseMessage) => void;
  reconnect: () => void;
}

export function useApprovalWebSocket(url: string | null): ApprovalWebSocketHookReturn {
  const [isConnected, setIsConnected] = useState(false);
  const [pendingRequests, setPendingRequests] = useState<ApprovalRequest[]>([]);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const reconnectAttemptsRef = useRef(0);

  const handleMessage = useCallback((event: MessageEvent) => {
    try {
      const parsedMessage = ApprovalRequestMessageSchema.parse(JSON.parse(event.data));
      
      // Convert the parsed message to internal ApprovalRequest format
      const approvalRequest: ApprovalRequest = {
        id: parsedMessage.id,
        tool_name: parsedMessage.request.tool_name,
        input: parsedMessage.request.input,
        permission_suggestions: parsedMessage.request.permission_suggestions as PermissionUpdate[] | undefined,
        created_at: parsedMessage.created_at
      };
      
      setPendingRequests(prev => {
        const exists = prev.find(req => req.id === approvalRequest.id);
        if (exists) return prev;
        return [...prev, approvalRequest];
      });
    } catch (err) {
      console.error('Failed to parse approval WebSocket message:', err);
    }
  }, []);

  const scheduleReconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) return;

    const delay = Math.min(1000 * Math.pow(2, reconnectAttemptsRef.current), 30000);
    reconnectAttemptsRef.current += 1;

    reconnectTimeoutRef.current = setTimeout(() => {
      reconnectTimeoutRef.current = null;
      if (url) {
        // Directly call connect logic here to avoid circular dependency
        try {
          const ws = new WebSocket(url);
          wsRef.current = ws;

          ws.onopen = () => {
            setIsConnected(true);
            setError(null);
            reconnectAttemptsRef.current = 0;
          };

          ws.onmessage = handleMessage;

          ws.onclose = () => {
            setIsConnected(false);
            scheduleReconnect();
          };

          ws.onerror = () => {
            setError('Approval WebSocket connection error');
            setIsConnected(false);
          };
        } catch {
          setError('Failed to create approval WebSocket connection');
          scheduleReconnect();
        }
      }
    }, delay);
  }, [url, handleMessage]);

  const connect = useCallback(() => {
    if (!url || wsRef.current?.readyState === WebSocket.CONNECTING) return;

    try {
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        setIsConnected(true);
        setError(null);
        reconnectAttemptsRef.current = 0;
      };

      ws.onmessage = handleMessage;

      ws.onclose = () => {
        setIsConnected(false);
        scheduleReconnect();
      };

      ws.onerror = () => {
        setError('Approval WebSocket connection error');
        setIsConnected(false);
      };
    } catch {
      setError('Failed to create approval WebSocket connection');
      scheduleReconnect();
    }
  }, [url, handleMessage, scheduleReconnect]);


  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setIsConnected(false);
  }, []);

  const sendApprovalResponse = useCallback((response: ApprovalResponseMessage) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(response));
      
      setPendingRequests(prev => 
        prev.filter(req => req.id !== response.id)
      );
    }
  }, []);

  const reconnect = useCallback(() => {
    disconnect();
    reconnectAttemptsRef.current = 0;
    connect();
  }, [disconnect, connect]);

  useEffect(() => {
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
    pendingRequests,
    error,
    sendApprovalResponse,
    reconnect,
  };
}