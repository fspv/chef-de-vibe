import { useState } from 'react';
import { CollapsibleContent } from './CollapsibleContent';
import { EditDiff } from './DiffViewer';

interface ControlRequestMessageProps {
  message: any; // Control request message
  timestamp?: number;
  onApprove?: (requestId: string, input: Record<string, unknown>, permissionUpdates?: Array<Record<string, unknown>>) => void;
  onDeny?: (requestId: string) => void;
}

export function ControlRequestMessage({ message, timestamp, onApprove, onDeny }: ControlRequestMessageProps) {
  const [selectedPermissions, setSelectedPermissions] = useState<boolean[]>(
    message.request.permission_suggestions ? 
    new Array(message.request.permission_suggestions.length).fill(false) : []
  );
  const [modifiedInput, setModifiedInput] = useState<string>(
    JSON.stringify(message.request.input, null, 2)
  );
  const [inputError, setInputError] = useState<string | null>(null);
  const [isProcessed, setIsProcessed] = useState(false);

  const { tool_name, input } = message.request;

  const handlePermissionToggle = (index: number) => {
    setSelectedPermissions(prev => {
      const newSelected = [...prev];
      newSelected[index] = !newSelected[index];
      return newSelected;
    });
  };

  const handleApprove = () => {
    if (onApprove) {
      try {
        let finalInput = input;
        
        // Check if input was modified
        if (modifiedInput.trim() !== JSON.stringify(input, null, 2).trim()) {
          finalInput = JSON.parse(modifiedInput);
        }
        
        // Get selected permissions
        const selectedPerms = message.request.permission_suggestions?.filter(
          (_: any, index: number) => selectedPermissions[index]
        );
        
        onApprove(message.request_id, finalInput, selectedPerms);
        setIsProcessed(true);
      } catch (err) {
        setInputError('Invalid JSON format');
      }
    }
  };

  const handleDeny = () => {
    if (onDeny) {
      onDeny(message.request_id);
      setIsProcessed(true);
    }
  };

  return (
    <div className={`control-request-message ${isProcessed ? 'processed' : ''}`}>
      <div className="message-role">
        ⚠️ Tool Approval Request
        {timestamp && (
          <span className="message-timestamp">
            {new Date(timestamp).toLocaleTimeString()}
          </span>
        )}
      </div>
      <div className="tool-approval-content">
        <div className="tool-name">🛠️ Tool: {tool_name}</div>
        
        {/* Show tool-specific content or editable JSON */}
        {tool_name === 'Edit' && input && typeof input === 'object' && 
         'file_path' in input && 'old_string' in input && 'new_string' in input ? (
          <>
            <EditDiff toolInput={input as any} />
            <details className="edit-json-details">
              <summary>Edit JSON (Advanced)</summary>
              <textarea
                value={modifiedInput}
                onChange={(e) => {
                  setModifiedInput(e.target.value);
                  setInputError(null);
                }}
                className="tool-input-editor"
                rows={8}
                disabled={isProcessed}
              />
              {inputError && <div className="input-error">{inputError}</div>}
            </details>
          </>
        ) : tool_name === 'Write' && input && typeof input === 'object' && 
           'file_path' in input && 'content' in input ? (
          <>
            <div className="write-tool-content">
              <div className="file-path">📄 File: {String(input.file_path)}</div>
              <CollapsibleContent 
                content={String(input.content)}
                className="file-content"
                maxLines={20}
                isCode={true}
              />
            </div>
            <details className="edit-json-details">
              <summary>Edit JSON (Advanced)</summary>
              <textarea
                value={modifiedInput}
                onChange={(e) => {
                  setModifiedInput(e.target.value);
                  setInputError(null);
                }}
                className="tool-input-editor"
                rows={8}
                disabled={isProcessed}
              />
              {inputError && <div className="input-error">{inputError}</div>}
            </details>
          </>
        ) : (
          <div className="tool-input-section">
            <h4>Tool Input:</h4>
            <textarea
              value={modifiedInput}
              onChange={(e) => {
                setModifiedInput(e.target.value);
                setInputError(null);
              }}
              className="tool-input-editor"
              rows={8}
              disabled={isProcessed}
            />
            {inputError && <div className="input-error">{inputError}</div>}
          </div>
        )}
        
        {/* Permission suggestions with checkboxes */}
        {message.request.permission_suggestions && message.request.permission_suggestions.length > 0 && (
          <div className="permission-suggestions-inline">
            <h4>Permission Suggestions:</h4>
            {message.request.permission_suggestions.map((suggestion: any, index: number) => (
              <div key={index} className="permission-checkbox-line">
                <input
                  type="checkbox"
                  id={`perm-${message.request_id}-${index}`}
                  checked={selectedPermissions[index]}
                  onChange={() => handlePermissionToggle(index)}
                  disabled={isProcessed}
                />
                <label htmlFor={`perm-${message.request_id}-${index}`}>
                  <code>{JSON.stringify(suggestion)}</code>
                </label>
              </div>
            ))}
          </div>
        )}
        
        {/* Approval buttons */}
        {!isProcessed && (
          <div className="approval-actions">
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
        )}
        
        {isProcessed && (
          <div className="approval-status">
            Request has been processed
          </div>
        )}
      </div>
    </div>
  );
}