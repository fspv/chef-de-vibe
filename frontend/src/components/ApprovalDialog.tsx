import { useState } from 'react';
import type { ApprovalRequest } from '../types/api';
import type { PermissionUpdate } from '@anthropic-ai/claude-code/sdk';

export interface ApprovalDialogProps {
  request: ApprovalRequest;
  onApprove: (
    wrapperId: string,
    originalInput: Record<string, unknown>,
    updatedInput?: Record<string, unknown>,
    updatedPermissions?: PermissionUpdate[]
  ) => void;
  onDeny: (wrapperId: string) => void;
  onClose: () => void;
}

export function ApprovalDialog({ request, onApprove, onDeny, onClose }: ApprovalDialogProps) {
  const [modifiedInput, setModifiedInput] = useState<string>(
    JSON.stringify(request.input, null, 2)
  );
  const [inputError, setInputError] = useState<string | null>(null);
  const [selectedPermissions, setSelectedPermissions] = useState<boolean[]>(
    request.permission_suggestions ? new Array(request.permission_suggestions.length).fill(false) : []
  );

  const handleApprove = () => {
    try {
      let updatedInput: Record<string, unknown> | undefined;
      
      if (modifiedInput.trim() !== JSON.stringify(request.input, null, 2).trim()) {
        updatedInput = JSON.parse(modifiedInput);
      }

      const selectedPermissionUpdates = request.permission_suggestions?.filter((_, index) => selectedPermissions[index]);

      onApprove(request.id, request.input, updatedInput, selectedPermissionUpdates);
      onClose();
    } catch {
      setInputError('Invalid JSON format');
    }
  };

  const handlePermissionToggle = (index: number) => {
    setSelectedPermissions(prev => {
      const newSelected = [...prev];
      newSelected[index] = !newSelected[index];
      return newSelected;
    });
  };

  const handleDeny = () => {
    onDeny(request.id);
    onClose();
  };

  const handleInputChange = (value: string) => {
    setModifiedInput(value);
    setInputError(null);
  };

  const formatCreatedAt = (timestamp: number) => {
    try {
      return new Date(timestamp * 1000).toLocaleString(); // Convert Unix timestamp to milliseconds
    } catch {
      return new Date(timestamp).toLocaleString(); // Fallback if already in milliseconds
    }
  };

  return (
    <div className="approval-dialog-overlay" onClick={onClose}>
      <div className="approval-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="approval-dialog-header">
          <h2>Tool Permission Request</h2>
          <button className="close-button" onClick={onClose}>×</button>
        </div>
        
        <div className="approval-dialog-content">
          <div className="tool-info">
            <h3>Tool: <code>{request.tool_name}</code></h3>
            <p className="request-time">Requested at: {formatCreatedAt(request.created_at)}</p>
          </div>

          <div className="tool-input-section">
            <h4>Tool Input:</h4>
            <textarea
              value={modifiedInput}
              onChange={(e) => handleInputChange(e.target.value)}
              className="tool-input-editor"
              rows={8}
              placeholder="Tool input parameters (JSON)"
            />
            {inputError && <div className="input-error">{inputError}</div>}
          </div>

          {request.permission_suggestions && request.permission_suggestions.length > 0 && (
            <div className="permission-suggestions">
              <h4>Permission Suggestions</h4>
              <div className="suggestions-content">
                {request.permission_suggestions.map((suggestion, index) => (
                  <div key={index} className="permission-suggestion">
                    <div className="permission-checkbox">
                      <input
                        type="checkbox"
                        id={`permission-${index}`}
                        checked={selectedPermissions[index]}
                        onChange={() => handlePermissionToggle(index)}
                      />
                      <label htmlFor={`permission-${index}`}>
                        <code className="permission-json">{JSON.stringify(suggestion)}</code>
                      </label>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        <div className="approval-dialog-actions">
          <button 
            className="deny-button"
            onClick={handleDeny}
          >
            Deny
          </button>
          <button 
            className="approve-button"
            onClick={handleApprove}
          >
            Approve
          </button>
        </div>
      </div>
    </div>
  );
}