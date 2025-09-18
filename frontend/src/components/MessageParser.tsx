import { 
  isSDKMessage,
  isSDKUserMessage,
  isSDKAssistantMessage,
  isSDKResultMessage,
  isSDKSystemMessage,
  isSDKPartialAssistantMessage,
  isSDKCompactBoundaryMessage,
  isSDKControlRequestMessage,
  isLikelyClaudeCodeMessage,
  type ExtendedSDKMessage,
  type AnyTodoItem
} from '../types/claude-messages';
import { CollapsibleContent } from './CollapsibleContent';
import { TodoList } from './TodoList';
import { EditDiff } from './DiffViewer';
import { isEditTool } from '../utils/diffUtils';
import { ControlRequestMessage } from './ControlRequestMessage';

// Helper to check if tool input is for Write tool
function isWriteTool(toolName: string, input: unknown): boolean {
  return toolName === 'Write' && 
         typeof input === 'object' && 
         input !== null &&
         'file_path' in input &&
         'content' in input;
}

// Component for displaying Write tool content
function WriteToolDisplay({ input }: { input: { file_path: string; content: string } }) {
  return (
    <div className="write-tool-display">
      <div className="write-tool-header">
        <h4>📝 Writing File</h4>
        <div className="file-path">📄 {input.file_path}</div>
      </div>
      <CollapsibleContent 
        content={input.content}
        className="file-content"
        maxLines={20}
        isCode={true}
      />
    </div>
  );
}

interface MessageParserProps {
  data: unknown;
  timestamp?: number;
  showRawJson: boolean;
  messageSource: 'session' | 'websocket';
  onApprove?: (requestId: string, input: Record<string, unknown>, permissionUpdates?: Array<Record<string, unknown>>) => void;
  onDeny?: (requestId: string) => void;
}

interface ParsedMessageResult {
  isClaudeMessage: boolean;
  messageType: string;
  message?: ExtendedSDKMessage;
  rawData: unknown;
}

function parseMessage(data: unknown): ParsedMessageResult {
  // Try to parse as Claude Code SDK message
  if (isSDKMessage(data)) {
    return {
      isClaudeMessage: true,
      messageType: getMessageTypeDescription(data),
      message: data,
      rawData: data
    };
  }

  // Check if it looks like a Claude Code message but failed parsing
  if (isLikelyClaudeCodeMessage(data)) {
    return {
      isClaudeMessage: true,
      messageType: 'Unknown Claude Message',
      rawData: data
    };
  }

  // Generic message
  return {
    isClaudeMessage: false,
    messageType: 'Generic Message',
    rawData: data
  };
}

function getMessageTypeDescription(message: ExtendedSDKMessage): string {
  if (isSDKUserMessage(message)) {
    return 'User Message';
  }
  if (isSDKAssistantMessage(message)) {
    return 'Assistant Message';
  }
  if (isSDKResultMessage(message)) {
    return `Result: ${message.subtype}`;
  }
  if (isSDKSystemMessage(message)) {
    if (message.subtype === 'init') {
      return 'System Initialization';
    }
    return 'System Message';
  }
  if (isSDKPartialAssistantMessage(message)) {
    return 'Streaming Assistant';
  }
  if (isSDKCompactBoundaryMessage(message)) {
    return 'Compact Boundary';
  }
  if (isSDKControlRequestMessage(message)) {
    return 'Tool Approval Request';
  }
  return 'Unknown Message';
}

function parseTodosFromToolUse(input: unknown): AnyTodoItem[] | null {
  if (typeof input !== 'object' || input === null) {
    return null;
  }
  
  const inputObj = input as Record<string, unknown>;
  
  if (!('todos' in inputObj) || !Array.isArray(inputObj.todos)) {
    return null;
  }
  
  return inputObj.todos.filter((todo: unknown): todo is AnyTodoItem => {
    if (typeof todo !== 'object' || todo === null) {
      return false;
    }
    
    const todoObj = todo as Record<string, unknown>;
    
    return (
      typeof todoObj.content === 'string' &&
      typeof todoObj.status === 'string' &&
      ['pending', 'in_progress', 'completed'].includes(todoObj.status)
    );
  });
}

export function MessageParser({ data, timestamp, showRawJson, onApprove, onDeny }: MessageParserProps) {
  const parsed = parseMessage(data);

  if (showRawJson) {
    return (
      <div className="message-item raw-json">
        <div className="message-header">
          <span className="message-type">Raw JSON</span>
          {timestamp && (
            <span className="message-timestamp">
              {new Date(timestamp).toLocaleTimeString()}
            </span>
          )}
        </div>
        <CollapsibleContent 
          content={JSON.stringify(data, null, 2)}
          className="message-content"
          maxLines={20}
          isCode={true}
        />
      </div>
    );
  }

  if (parsed.isClaudeMessage && parsed.message) {
    try {
      return <FormattedClaudeMessage message={parsed.message} timestamp={timestamp} onApprove={onApprove} onDeny={onDeny} />;
    } catch (error) {
      console.error('Error rendering Claude message:', error, 'Message data:', parsed.message);
      return (
        <div className="message-error fallback">
          <div className="message-header">
            <span className="message-type">❌ Parse Error - {parsed.messageType}</span>
            {timestamp && (
              <span className="message-timestamp">
                {new Date(timestamp).toLocaleTimeString()}
              </span>
            )}
          </div>
          <div className="error-details">
            <div className="error-message">
              Failed to render message: {error instanceof Error ? error.message : String(error)}
            </div>
            <CollapsibleContent 
              content={JSON.stringify(parsed.rawData, null, 2)}
              className="error-raw-json"
              maxLines={20}
              isCode={true}
            />
          </div>
        </div>
      );
    }
  }

  return (
    <div className="generic-message">
      <div className="message-header">
        <span className="message-type">{parsed.messageType}</span>
        {timestamp && (
          <span className="message-timestamp">
            {new Date(timestamp).toLocaleTimeString()}
          </span>
        )}
      </div>
      <CollapsibleContent 
        content={JSON.stringify(parsed.rawData, null, 2)}
        className="generic-message-content"
        maxLines={15}
        isCode={true}
      />
    </div>
  );
}

function FormattedClaudeMessage({ message, timestamp, onApprove, onDeny }: { 
  message: ExtendedSDKMessage; 
  timestamp?: number;
  onApprove?: (requestId: string, input: Record<string, unknown>, permissionUpdates?: Array<Record<string, unknown>>) => void;
  onDeny?: (requestId: string) => void;
}) {
  if (isSDKUserMessage(message)) {
    return (
      <div className="user-message">
        <div className="message-role">
          👤 User
          {timestamp && (
            <span className="message-timestamp">
              {new Date(timestamp).toLocaleTimeString()}
            </span>
          )}
        </div>
        <div className="message-content-blocks">
          {Array.isArray(message.message.content) ? (
            message.message.content.map((block: Record<string, unknown>, index: number) => (
              <div key={index} className={`content-block ${block.type || 'unknown'}`}>
                {block.type === 'text' && (
                  <CollapsibleContent 
                    content={String(block.text)} 
                    className="text-content"
                    maxLines={10}
                  />
                )}
                {block.type === 'tool_result' && (
                  <div className="tool-result-content">
                    <div className="tool-result-header">
                      🔧 Tool Result
                      {block.tool_use_id ? (
                        <span className="tool-use-id">ID: {String(block.tool_use_id)}</span>
                      ) : null}
                    </div>
                    <CollapsibleContent 
                      content={typeof block.content === 'string' ? block.content : JSON.stringify(block.content, null, 2)}
                      className="tool-result-text"
                      maxLines={15}
                      isCode={true}
                    />
                  </div>
                )}
                {block.type === 'tool_use' && (
                  <div className="tool-use-content">
                    <div className="tool-name">🛠️ {String(block.name)}</div>
                    <div className="tool-id">ID: {String(block.id)}</div>
                    {String(block.name) === 'TodoWrite' ? (
                      (() => {
                        const todos = parseTodosFromToolUse(block.input);
                        return todos ? (
                          <TodoList todos={todos} />
                        ) : (
                          <CollapsibleContent 
                            content={JSON.stringify(block.input, null, 2)}
                            className="tool-input"
                            maxLines={10}
                            isCode={true}
                          />
                        );
                      })()
                    ) : isEditTool(String(block.name), block.input) ? (
                      <EditDiff toolInput={block.input} />
                    ) : isWriteTool(String(block.name), block.input) ? (
                      <WriteToolDisplay input={block.input} />
                    ) : (
                      <CollapsibleContent 
                        content={JSON.stringify(block.input, null, 2)}
                        className="tool-input"
                        maxLines={10}
                        isCode={true}
                      />
                    )}
                  </div>
                )}
                {!['text', 'tool_result', 'tool_use'].includes(String(block.type)) && (
                  <div className="unknown-content-block">
                    <div className="block-type">Unknown type: {String(block.type)}</div>
                    <CollapsibleContent 
                      content={JSON.stringify(block, null, 2)}
                      className="block-content"
                      maxLines={10}
                      isCode={true}
                    />
                  </div>
                )}
              </div>
            ))
          ) : (
            <CollapsibleContent 
              content={typeof message.message.content === 'string' 
                ? message.message.content 
                : JSON.stringify(message.message.content, null, 2)}
              className="message-text"
              maxLines={10}
              isCode={typeof message.message.content !== 'string'}
            />
          )}
        </div>
        {message.parent_tool_use_id && (
          <div className="parent-tool-id">
            Tool Use ID: {message.parent_tool_use_id}
          </div>
        )}
      </div>
    );
  }

  if (isSDKAssistantMessage(message)) {
    return (
      <div className="assistant-message">
        <div className="message-role">
          🤖 Assistant
          {timestamp && (
            <span className="message-timestamp">
              {new Date(timestamp).toLocaleTimeString()}
            </span>
          )}
        </div>
        <div className="message-content-blocks">
          {message.message.content.map((block: Record<string, unknown>, index: number) => (
            <div key={index} className={`content-block ${block.type}`}>
              {block.type === 'text' && (
                <CollapsibleContent 
                  content={String(block.text)} 
                  className="text-content"
                  maxLines={10}
                />
              )}
              {block.type === 'tool_use' && (
                <div className="tool-use-content">
                  <div className="tool-name">🛠️ {String(block.name)}</div>
                  <div className="tool-id">ID: {String(block.id)}</div>
                  {String(block.name) === 'TodoWrite' ? (
                    (() => {
                      const todos = parseTodosFromToolUse(block.input);
                      return todos ? (
                        <TodoList todos={todos} />
                      ) : (
                        <CollapsibleContent 
                          content={JSON.stringify(block.input, null, 2)}
                          className="tool-input"
                          maxLines={10}
                          isCode={true}
                        />
                      );
                    })()
                  ) : isEditTool(String(block.name), block.input) ? (
                    <EditDiff toolInput={block.input} />
                  ) : isWriteTool(String(block.name), block.input) ? (
                    <WriteToolDisplay input={block.input} />
                  ) : (
                    <CollapsibleContent 
                      content={JSON.stringify(block.input, null, 2)}
                      className="tool-input"
                      maxLines={10}
                      isCode={true}
                    />
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (isSDKResultMessage(message)) {
    const isSuccess = message.subtype === 'success';
    return (
      <div className={`result-message ${isSuccess ? 'success' : 'error'}`}>
        <div className="message-role">
          {isSuccess ? '✅ Success' : '❌ Error'}
        </div>
        <div className="result-details">
          <div className="result-metrics">
            <span>Duration: {message.duration_ms}ms</span>
            <span>API Time: {message.duration_api_ms}ms</span>
            <span>Turns: {message.num_turns}</span>
            <span>Cost: ${message.total_cost_usd.toFixed(4)}</span>
          </div>
          {isSuccess && message.subtype === 'success' && (
            <CollapsibleContent 
              content={message.result}
              className="result-text"
              maxLines={15}
            />
          )}
          {message.permission_denials && message.permission_denials.length > 0 && (
            <div className="permission-denials">
              <h4>Permission Denials:</h4>
              {message.permission_denials.map((denial, index) => (
                <div key={index} className="permission-denial">
                  Tool: {denial.tool_name} (ID: {denial.tool_use_id})
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }

  if (isSDKSystemMessage(message)) {
    return (
      <div className="system-message">
        <div className="message-role">⚙️ System</div>
        <div className="system-details">
          <div className="system-info">
            <div>Model: {message.model}</div>
            <div>Permission Mode: {message.permissionMode}</div>
            <div>API Key Source: {message.apiKeySource}</div>
            <div>Working Directory: {message.cwd}</div>
          </div>
          {message.tools && message.tools.length > 0 && (
            <div className="tools-section">
              <h4>Available Tools ({message.tools.length}):</h4>
              <div className="tools-list">
                {message.tools.map((tool, index) => (
                  <span key={index} className="tool-tag">{tool}</span>
                ))}
              </div>
            </div>
          )}
          {message.mcp_servers && message.mcp_servers.length > 0 && (
            <div className="mcp-servers">
              <h4>MCP Servers:</h4>
              {message.mcp_servers.map((server, index) => (
                <div key={index} className="mcp-server">
                  {server.name} ({server.status})
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }

  if (isSDKPartialAssistantMessage(message)) {
    return (
      <div className="streaming-message">
        <div className="message-role">🔄 Streaming</div>
        <div className="stream-event">
          Event Type: {message.event.type}
        </div>
      </div>
    );
  }

  if (isSDKCompactBoundaryMessage(message)) {
    return (
      <div className="compact-boundary">
        <div className="message-role">📦 Compact Boundary</div>
        <div className="compact-info">
          <div>Trigger: {message.compact_metadata.trigger}</div>
          <div>Pre-tokens: {message.compact_metadata.pre_tokens}</div>
        </div>
      </div>
    );
  }

  if (isSDKControlRequestMessage(message)) {
    return <ControlRequestMessage message={message} timestamp={timestamp} onApprove={onApprove} onDeny={onDeny} />;
  }

  // Fallback for unknown message types
  return (
    <CollapsibleContent 
      content={JSON.stringify(message, null, 2)}
      className="unknown-message"
      maxLines={15}
      isCode={true}
    />
  );
}

