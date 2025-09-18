import { useState } from 'react';
import { EditDiff } from './DiffViewer';
import { 
  WriteToolDisplay, 
  AgentToolDisplay, 
  BashToolDisplay, 
  BashOutputToolDisplay,
  ExitPlanModeToolDisplay,
  FileMultiEditToolDisplay,
  FileReadToolDisplay,
  GlobToolDisplay,
  GrepToolDisplay,
  KillShellToolDisplay,
  ListMcpResourcesToolDisplay,
  McpToolDisplay,
  NotebookEditToolDisplay,
  ReadMcpResourceToolDisplay,
  WebFetchToolDisplay,
  WebSearchToolDisplay
} from './MessageParser';
import type { PermissionUpdate } from '@anthropic-ai/claude-code/sdk';
import type { 
  FileEditInput, 
  ToolInputSchemas,
  FileWriteInput,
  ExitPlanModeInput,
  AgentInput,
  BashInput,
  BashOutputInput,
  FileMultiEditInput,
  FileReadInput,
  GlobInput,
  GrepInput,
  KillShellInput,
  ListMcpResourcesInput,
  McpInput,
  NotebookEditInput,
  ReadMcpResourceInput,
  WebFetchInput,
  WebSearchInput
} from '@anthropic-ai/claude-code/sdk-tools';

// Helper function to render tool-specific content
function renderToolContent(toolName: string, input: ToolInputSchemas) {
  switch (toolName) {
    case 'ExitPlanMode':
      return <ExitPlanModeToolDisplay input={input as ExitPlanModeInput} />;
    case 'Task':
      return <AgentToolDisplay input={input as AgentInput} />;
    case 'Bash':
      return <BashToolDisplay input={input as BashInput} />;
    case 'BashOutput':
      return <BashOutputToolDisplay input={input as BashOutputInput} />;
    case 'Write':
      return <WriteToolDisplay input={input as FileWriteInput} />;
    case 'Edit':
      return <EditDiff toolInput={input as FileEditInput} />;
    case 'MultiEdit':
      return <FileMultiEditToolDisplay input={input as FileMultiEditInput} />;
    case 'Read':
      return <FileReadToolDisplay input={input as FileReadInput} />;
    case 'Glob':
      return <GlobToolDisplay input={input as GlobInput} />;
    case 'Grep':
      return <GrepToolDisplay input={input as GrepInput} />;
    case 'KillShell':
      return <KillShellToolDisplay input={input as KillShellInput} />;
    case 'ListMcpResources':
      return <ListMcpResourcesToolDisplay input={input as ListMcpResourcesInput} />;
    case 'Mcp':
      return <McpToolDisplay input={input as McpInput} />;
    case 'NotebookEdit':
      return <NotebookEditToolDisplay input={input as NotebookEditInput} />;
    case 'ReadMcpResource':
      return <ReadMcpResourceToolDisplay input={input as ReadMcpResourceInput} />;
    case 'WebFetch':
      return <WebFetchToolDisplay input={input as WebFetchInput} />;
    case 'WebSearch':
      return <WebSearchToolDisplay input={input as WebSearchInput} />;
    default:
      return null;
  }
}

interface ControlRequestMessage {
  type: string;
  request_id: string;
  request: {
    subtype: string;
    tool_name?: string;
    input?: ToolInputSchemas;
    permission_suggestions?: PermissionUpdate[];
  };
}

interface ControlRequestMessageProps {
  message: ControlRequestMessage;
  timestamp?: number;
  onApprove?: (requestId: string, input: ToolInputSchemas, permissionUpdates: PermissionUpdate[]) => void;
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
  
  // Only show tool approval UI for can_use_tool requests
  if (message.request.subtype !== 'can_use_tool' || !tool_name || !input) {
    return (
      <div className="control-request-message">
        <div className="message-role">
          ⚠️ Control Request
          {timestamp && (
            <span className="message-timestamp">
              {new Date(timestamp).toLocaleTimeString()}
            </span>
          )}
        </div>
        <div className="control-request-content">
          <div className="control-subtype">Type: {message.request.subtype}</div>
          <pre>{JSON.stringify(message.request, null, 2)}</pre>
        </div>
      </div>
    );
  }

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
          (_, index: number) => selectedPermissions[index]
        ) || [];
        
        onApprove(message.request_id, finalInput, selectedPerms);
        setIsProcessed(true);
      } catch {
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
        
        {/* Show tool-specific content */}
        {(() => {
          const toolContent = renderToolContent(tool_name, input);
          
          if (toolContent) {
            return (
              <>
                {toolContent}
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
            );
          } else {
            return (
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
            );
          }
        })()}
        
        {/* Permission suggestions with checkboxes */}
        {message.request.permission_suggestions && message.request.permission_suggestions.length > 0 && (
          <div className="permission-suggestions-inline">
            <h4>Permission Suggestions:</h4>
            {message.request.permission_suggestions.map((suggestion: PermissionUpdate, index: number) => (
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
