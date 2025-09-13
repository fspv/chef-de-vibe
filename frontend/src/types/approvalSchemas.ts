import { z } from 'zod';
import type { PermissionUpdate, PermissionResult } from '@anthropic-ai/claude-code/sdk';

// Schema for Claude's raw approval request content (from request.request field)
// Note: We don't validate the exact structure of PermissionUpdate since it's from the library
export const ClaudeRequestSchema = z.object({
  subtype: z.literal('can_use_tool'),
  tool_name: z.string(),
  input: z.record(z.string(), z.unknown()),
  permission_suggestions: z.array(z.unknown()).optional(), // Let TypeScript handle the PermissionUpdate[] typing
});

// Schema for the wrapped approval request message from backend
export const ApprovalRequestMessageSchema = z.object({
  id: z.string(),
  request: ClaudeRequestSchema,
  created_at: z.number(), // Backend sends Unix timestamp as number
});

// No schema needed - using PermissionResult directly from Claude Code library

// Inferred types from schemas, with proper PermissionUpdate typing
export type ClaudeRequest = z.infer<typeof ClaudeRequestSchema> & {
  permission_suggestions?: PermissionUpdate[];
};

export type ApprovalRequestMessage = z.infer<typeof ApprovalRequestMessageSchema> & {
  request: ClaudeRequest;
};

// Simple wrapper for approval responses using Claude Code library's PermissionResult
export type ApprovalResponseMessage = {
  id: string;
  response: PermissionResult;
};