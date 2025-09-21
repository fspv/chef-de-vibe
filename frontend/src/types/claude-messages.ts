// Claude Code SDK message types
// Based on @anthropic-ai/claude-code SDK types

import type { 
  SDKMessage, 
  SDKUserMessage, 
  SDKAssistantMessage, 
  SDKResultMessage, 
  SDKSystemMessage,
  SDKPartialAssistantMessage,
  SDKCompactBoundaryMessage,
  PermissionUpdate
} from '@anthropic-ai/claude-code';

// Re-export SDK types for use in components
export type {
  SDKMessage,
  SDKUserMessage,
  SDKAssistantMessage,
  SDKResultMessage,
  SDKSystemMessage,
  SDKPartialAssistantMessage,
  SDKCompactBoundaryMessage
};

// Extended SDKMessage type to include control requests, responses, and summaries
export type ExtendedSDKMessage = SDKMessage | SDKControlRequestMessage | SDKControlResponseMessage | SDKSummaryMessage;

// Extended todo types to handle actual usage patterns
export interface TodoItem {
  content: string;
  status: "pending" | "in_progress" | "completed";
  id: string;
  priority: "high" | "medium" | "low";
}

// Official claude-code todo item (from TodoWriteInput)
export interface OfficialTodoItem {
  content: string;
  status: "pending" | "in_progress" | "completed";
  activeForm: string;
}

// Union type to handle both formats
export type AnyTodoItem = TodoItem | OfficialTodoItem;

// Type guard to check if a todo item has id/priority fields (extended format)
export function isExtendedTodoItem(item: AnyTodoItem): item is TodoItem {
  return 'id' in item && 'priority' in item;
}

// Type guard to check if a todo item has activeForm field (official format)
export function isOfficialTodoItem(item: AnyTodoItem): item is OfficialTodoItem {
  return 'activeForm' in item;
}

// Type guards for message identification
export function isSDKUserMessage(message: unknown): message is SDKUserMessage {
  return typeof message === 'object' && 
         message !== null && 
         'type' in message && 
         message.type === 'user';
}

export function isSDKAssistantMessage(message: unknown): message is SDKAssistantMessage {
  return typeof message === 'object' && 
         message !== null && 
         'type' in message && 
         message.type === 'assistant';
}

export function isSDKResultMessage(message: unknown): message is SDKResultMessage {
  return typeof message === 'object' && 
         message !== null && 
         'type' in message && 
         message.type === 'result';
}

export function isSDKSystemMessage(message: unknown): message is SDKSystemMessage {
  return typeof message === 'object' && 
         message !== null && 
         'type' in message && 
         message.type === 'system';
}

export function isSDKPartialAssistantMessage(message: unknown): message is SDKPartialAssistantMessage {
  return typeof message === 'object' && 
         message !== null && 
         'type' in message && 
         message.type === 'stream_event';
}

export function isSDKCompactBoundaryMessage(message: unknown): message is SDKCompactBoundaryMessage {
  return typeof message === 'object' && 
         message !== null && 
         'type' in message && 
         message.type === 'system' &&
         'subtype' in message &&
         message.subtype === 'compact_boundary';
}

export function isSDKMessage(message: unknown): message is SDKMessage | SDKControlRequestMessage | SDKControlResponseMessage | SDKSummaryMessage {
  return isSDKUserMessage(message) ||
         isSDKAssistantMessage(message) ||
         isSDKResultMessage(message) ||
         isSDKSystemMessage(message) ||
         isSDKPartialAssistantMessage(message) ||
         isSDKCompactBoundaryMessage(message) ||
         isSDKControlRequestMessage(message) ||
         isSDKControlResponseMessage(message) ||
         isSDKSummaryMessage(message);
}

// Control request message type (for approval requests and other control operations)
export interface SDKControlRequestMessage {
  type: 'control_request';
  request_id: string;
  request: {
    subtype: 'can_use_tool' | 'set_permission_mode' | 'interrupt' | 'set_model';
    tool_name?: string;
    input?: Record<string, unknown>;
    permission_suggestions?: PermissionUpdate[];
    mode?: string;
    model?: string;
  };
}

// Control response message type (for control request responses)
export interface SDKControlResponseMessage {
  type: 'control_response';
  response: {
    subtype: 'success' | 'error';
    request_id: string;
    response?: Record<string, unknown>;
    error?: string;
  };
}

// Summary message type (for generated summaries)
export interface SDKSummaryMessage {
  type: 'summary';
  leafUuid: string;
  summary: string;
}

// Type guard for control request messages
export function isSDKControlRequestMessage(message: unknown): message is SDKControlRequestMessage {
  if (typeof message !== 'object' || message === null) {
    return false;
  }
  
  const obj = message as Record<string, unknown>;
  
  return (
    obj.type === 'control_request' &&
    typeof obj.request === 'object' &&
    obj.request !== null &&
    typeof (obj.request as Record<string, unknown>).subtype === 'string'
  );
}

// Type guard for summary messages
export function isSDKSummaryMessage(message: unknown): message is SDKSummaryMessage {
  if (typeof message !== 'object' || message === null) {
    return false;
  }
  
  const obj = message as Record<string, unknown>;
  
  return (
    obj.type === 'summary' &&
    typeof obj.leafUuid === 'string' &&
    typeof obj.summary === 'string'
  );
}

// Type guard for control response messages
export function isSDKControlResponseMessage(message: unknown): message is SDKControlResponseMessage {
  if (typeof message !== 'object' || message === null) {
    return false;
  }
  
  const obj = message as Record<string, unknown>;
  
  return obj.type === 'control_response';
}

// Helper function to detect if raw data might be a Claude Code message
export function isLikelyClaudeCodeMessage(data: unknown): boolean {
  if (typeof data !== 'object' || data === null) {
    return false;
  }
  
  const obj = data as Record<string, unknown>;
  
  // Check for key indicators of Claude Code messages
  return (
    'type' in obj &&
    ('session_id' in obj || 'sessionId' in obj || obj.type === 'control_request' || obj.type === 'control_response' || obj.type === 'summary') &&
    (obj.type === 'user' || 
     obj.type === 'assistant' || 
     obj.type === 'result' || 
     obj.type === 'system' ||
     obj.type === 'stream_event' ||
     obj.type === 'control_request' ||
     obj.type === 'control_response' ||
     obj.type === 'summary')
  );
}