import type { ApprovalRequest, ApprovalResponseMessage } from '../types/api';
import type { PermissionUpdate, PermissionResult } from '@anthropic-ai/claude-code/sdk';

export interface ApprovalManager {
  pendingRequests: ApprovalRequest[];
  isConnected: boolean;
  error: string | null;
  approveRequest: (
    wrapperId: string,
    originalInput: Record<string, unknown>,
    updatedInput?: Record<string, unknown>,
    updatedPermissions?: PermissionUpdate[]
  ) => void;
  denyRequest: (wrapperId: string) => void;
  reconnect: () => void;
}

export interface ApprovalManagerState {
  activeRequest: ApprovalRequest | null;
  setActiveRequest: (request: ApprovalRequest | null) => void;
}

export class ApprovalManagerImpl implements ApprovalManager {
  constructor(
    public pendingRequests: ApprovalRequest[],
    public isConnected: boolean,
    public error: string | null,
    private sendApprovalResponse: (response: ApprovalResponseMessage) => void,
    public reconnect: () => void
  ) {}

  approveRequest(
    wrapperId: string,
    originalInput: Record<string, unknown>,
    updatedInput?: Record<string, unknown>,
    updatedPermissions?: PermissionUpdate[]
  ): void {
    const permissionResult: PermissionResult = {
      behavior: 'allow',
      updatedInput: updatedInput || originalInput,
      ...(updatedPermissions && { updatedPermissions }),
    };

    const response: ApprovalResponseMessage = {
      id: wrapperId,
      response: permissionResult,
    };

    this.sendApprovalResponse(response);
  }

  denyRequest(wrapperId: string): void {
    const permissionResult: PermissionResult = {
      behavior: 'deny',
      message: 'Tool usage denied by user',
    };

    const response: ApprovalResponseMessage = {
      id: wrapperId,
      response: permissionResult,
    };

    this.sendApprovalResponse(response);
  }
}

export function createApprovalManager(
  pendingRequests: ApprovalRequest[],
  isConnected: boolean,
  error: string | null,
  sendApprovalResponse: (response: ApprovalResponseMessage) => void,
  reconnect: () => void
): ApprovalManager {
  return new ApprovalManagerImpl(
    pendingRequests,
    isConnected,
    error,
    sendApprovalResponse,
    reconnect
  );
}