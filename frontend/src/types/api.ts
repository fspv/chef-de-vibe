import type { PermissionUpdate } from '@anthropic-ai/claude-code/sdk';

export interface Session {
  session_id: string;
  working_directory: string;
  active: boolean;
  summary?: string;
  earliest_message_date?: string;
  latest_message_date?: string;
}

export interface SessionsResponse {
  sessions: Session[];
}

export interface CreateSessionRequest {
  session_id: string;
  working_dir: string;
  resume: boolean;
  first_message: string;
}

export interface CreateSessionResponse {
  session_id: string;
  websocket_url: string;
  approval_websocket_url: string;
}

export interface Message {
  type: 'user' | 'assistant';
  message: UserMessage | AssistantMessage;
}

export interface UserMessage {
  role: 'user';
  content: string;
}

export interface AssistantMessage {
  role: 'assistant';
  content: AssistantContentBlock[];
}

export interface AssistantContentBlock {
  type: 'text';
  text: string;
}

export interface SessionDetailsResponse {
  session_id: string;
  content: Message[];
  websocket_url?: string;
  approval_websocket_url?: string;
  working_directory?: string;
}

export interface ApiError {
  error: string;
  code: string;
}

export type ApiResponse<T> = T | ApiError;

export function isApiError(response: unknown): response is ApiError {
  return typeof response === 'object' && response !== null && 'error' in response && 'code' in response;
}


// Re-export types from Zod schemas
export type { ApprovalRequestMessage, ApprovalResponseMessage } from './approvalSchemas';

// Parsed ApprovalRequest for internal use (extracted from raw Claude request)
export interface ApprovalRequest {
  id: string;  // Our wrapper ID for frontend/backend communication
  tool_name: string;
  input: Record<string, unknown>;  // Claude uses "input", not "tool_input"
  permission_suggestions?: PermissionUpdate[];
  created_at: number;  // Unix timestamp as number
}

// ApprovalWebSocketMessage is now just ApprovalRequestMessage (no more batch messages)