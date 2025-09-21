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
import type { PermissionUpdate, PermissionMode } from '@anthropic-ai/claude-code/sdk';
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
  onApprove?: (requestId: string, input: ToolInputSchemas, permissionUpdates: PermissionUpdate[]) => Promise<void> | void;
  onDeny?: (requestId: string) => Promise<void> | void;
  onModeChange?: (mode: PermissionMode) => void; // Still passed but not used for "Approve for Session"
}

export function ControlRequestMessage({ message, timestamp, onApprove, onDeny, onModeChange }: ControlRequestMessageProps) {
  // Check for setMode permission and extract it
  const setModePermission = message.request.permission_suggestions?.find(
    (perm: PermissionUpdate) => (perm as { type?: string; destination?: string }).type === 'setMode' && (perm as { type?: string; destination?: string }).destination === 'session'
  );
  
  // Filter out setMode permission from regular permissions
  const regularPermissions = message.request.permission_suggestions?.filter(
    (perm: PermissionUpdate) => !((perm as { type?: string; destination?: string }).type === 'setMode' && (perm as { type?: string; destination?: string }).destination === 'session')
  ) || [];
  
  const [selectedPermissions, setSelectedPermissions] = useState<boolean[]>(
    regularPermissions.length > 0 ? new Array(regularPermissions.length).fill(false) : []
  );
  const [modifiedInput, setModifiedInput] = useState<string>(
    JSON.stringify(message.request.input, null, 2)
  );
  const [inputError, setInputError] = useState<string | null>(null);
  const [isProcessed, setIsProcessed] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [loadingAction, setLoadingAction] = useState<'approve' | 'deny' | null>(null);
  const [sendError, setSendError] = useState<string | null>(null);

  const { tool_name, input } = message.request;
  
  // Only show tool approval UI for can_use_tool requests
  if (message.request.subtype !== 'can_use_tool' || !tool_name || !input) {
    // For set_permission_mode requests, show minimal grey text
    if (message.request.subtype === 'set_permission_mode') {
      const mode = (message.request as { mode?: string }).mode;
      return (
        <div className="control-response-message">
          <span className="response-id">Set mode: {mode || 'unknown'}</span>
        </div>
      );
    }
    
    // For interrupt requests, show minimal grey text
    if (message.request.subtype === 'interrupt') {
      return (
        <div className="control-response-message">
          <span className="response-id">→ Interrupt</span>
        </div>
      );
    }
    
    // For other control requests, show minimal info
    return (
      <div className="control-response-message">
        <span className="response-id">→ {message.request.subtype}</span>
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

  const handleApprove = async (includeSetMode = false) => {
    if (onApprove) {
      try {
        let finalInput = input;
        
        // Check if input was modified
        if (modifiedInput.trim() !== JSON.stringify(input, null, 2).trim()) {
          finalInput = JSON.parse(modifiedInput);
        }
        
        // Get selected permissions from regular permissions
        const selectedPerms = regularPermissions.filter(
          (_, index: number) => selectedPermissions[index]
        );
        
        // For ExitPlanMode, always include switching to default mode
        let permissionsToSend = selectedPerms;
        if (tool_name === 'ExitPlanMode') {
          // Add mode switch to default when approving ExitPlanMode
          const modeChangePermission: PermissionUpdate = {
            type: 'setMode',
            destination: 'session',
            mode: 'default'
          } as PermissionUpdate;
          permissionsToSend = [...selectedPerms, modeChangePermission];
        } else if (includeSetMode && setModePermission) {
          // Add setMode permission if requested for other tools
          permissionsToSend = [...selectedPerms, setModePermission];
        }
        
        setIsLoading(true);
        setLoadingAction('approve');
        setSendError(null);
        
        try {
          await onApprove(message.request_id, finalInput, permissionsToSend);
          setIsProcessed(true);
          
          // For ExitPlanMode, immediately update the UI to show mode change
          if (tool_name === 'ExitPlanMode') {
            onModeChange?.('default');
          } else if (includeSetMode && setModePermission) {
            // For "Approve for Session", update the mode based on the permission
            const mode = (setModePermission as { mode?: string }).mode;
            if (mode) {
              onModeChange?.(mode as PermissionMode);
            }
          }
        } catch (error) {
          setSendError(`Failed to send approval: ${error instanceof Error ? error.message : 'Unknown error'}`);
        } finally {
          setIsLoading(false);
          setLoadingAction(null);
        }
      } catch {
        setInputError('Invalid JSON format');
      }
    }
  };

  const handleDeny = async () => {
    if (onDeny) {
      setIsLoading(true);
      setLoadingAction('deny');
      setSendError(null);
      
      try {
        await onDeny(message.request_id);
        setIsProcessed(true);
      } catch (error) {
        setSendError(`Failed to send denial: ${error instanceof Error ? error.message : 'Unknown error'}`);
      } finally {
        setIsLoading(false);
        setLoadingAction(null);
      }
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
        
        {/* Permission suggestions with checkboxes (excluding setMode) */}
        {regularPermissions.length > 0 && (
          <div className="permission-suggestions-inline">
            <h4>Permission Suggestions:</h4>
            {regularPermissions.map((suggestion: PermissionUpdate, index: number) => (
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
              disabled={isLoading}
            >
              {isLoading && loadingAction === 'deny' ? (
                <span className="loading-spinner">⏳</span>
              ) : (
                'Deny'
              )}
            </button>
            <button 
              className="approve-button"
              onClick={() => handleApprove(false)}
              disabled={isLoading}
            >
              {isLoading && loadingAction === 'approve' ? (
                <span className="loading-spinner">⏳</span>
              ) : (
                'Approve'
              )}
            </button>
            {setModePermission && (
              <button 
                className="approve-session-button"
                onClick={() => handleApprove(true)}
                disabled={isLoading}
                title="Approve and switch to Accept Edits mode for this session"
              >
                {isLoading && loadingAction === 'approve' ? (
                  <span className="loading-spinner">⏳</span>
                ) : (
                  'Approve for Session'
                )}
              </button>
            )}
          </div>
        )}
        
        {sendError && (
          <div className="approval-error">
            {sendError}
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
