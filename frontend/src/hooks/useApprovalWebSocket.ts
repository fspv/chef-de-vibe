import { useState, useEffect, useCallback, useRef } from 'react';
import type { ApprovalRequest, ApprovalResponseMessage } from '../types/api';
import type { PermissionUpdate } from '@anthropic-ai/claude-code/sdk';
import { ApprovalRequestMessageSchema } from '../types/approvalSchemas';

export interface ApprovalWebSocketHookReturn {
  isConnected: boolean;
  pendingRequests: ApprovalRequest[];
  approvalMessages: Array<{data: unknown; timestamp: number}>;
  error: string | null;
  sendApprovalResponse: (response: ApprovalResponseMessage) => Promise<void>;
  reconnect: () => void;
}

export function useApprovalWebSocket(url: string | null): ApprovalWebSocketHookReturn {
  const [isConnected, setIsConnected] = useState(false);
  const [pendingRequests, setPendingRequests] = useState<ApprovalRequest[]>([]);
  const [approvalMessages, setApprovalMessages] = useState<Array<{data: unknown; timestamp: number}>>([]);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const reconnectAttemptsRef = useRef(0);

  const handleMessage = useCallback((event: MessageEvent) => {
    try {
      const rawData = JSON.parse(event.data);
      const parsedMessage = ApprovalRequestMessageSchema.parse(rawData);
      
      // Convert the parsed message to control_request format for display
      const controlRequestMessage = {
        type: 'control_request',
        request_id: parsedMessage.id,
        request: {
          subtype: 'can_use_tool',
          tool_name: parsedMessage.request.tool_name,
          input: parsedMessage.request.input,
          permission_suggestions: parsedMessage.request.permission_suggestions
        }
      };
      
      // Add to approval messages for display
      setApprovalMessages(prev => [...prev, { 
        data: controlRequestMessage, 
        timestamp: Date.now() 
      }]);
      
      // Also keep the approval request for handling
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

  const sendApprovalResponse = useCallback((response: ApprovalResponseMessage): Promise<void> => {
    return new Promise((resolve, reject) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        try {
          wsRef.current.send(JSON.stringify(response));
          
          // Remove from pending requests
          setPendingRequests(prev => 
            prev.filter(req => req.id !== response.id)
          );
          
          // Remove from approval messages display
          setApprovalMessages(prev =>
            prev.filter(msg => {
              const data = msg.data as { request_id?: string };
              return data.request_id !== response.id;
            })
          );
          resolve();
        } catch (error) {
          reject(error);
        }
      } else {
        reject(new Error('WebSocket is not connected'));
      }
    });
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
    approvalMessages,
    error,
    sendApprovalResponse,
    reconnect,
  };
}