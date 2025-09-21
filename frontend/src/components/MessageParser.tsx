import { 
  isSDKMessage,
  isSDKUserMessage,
  isSDKAssistantMessage,
  isSDKResultMessage,
  isSDKSystemMessage,
  isSDKPartialAssistantMessage,
  isSDKCompactBoundaryMessage,
  isSDKControlRequestMessage,
  isSDKControlResponseMessage,
  isSDKSummaryMessage,
  isLikelyClaudeCodeMessage,
  type ExtendedSDKMessage,
  type AnyTodoItem
} from '../types/claude-messages';
import type { PermissionUpdate, PermissionMode } from '@anthropic-ai/claude-code/sdk';
import { CollapsibleContent } from './CollapsibleContent';
import { TodoList } from './TodoList';
import { EditDiff } from './DiffViewer';
import { ControlRequestMessage } from './ControlRequestMessage';
import { MessageInfoButton } from './MessageInfoButton';
import { ToolInfoButton } from './ToolInfoButton';
import { MarkdownContent } from './MarkdownContent';
import type { 
  ToolInputSchemas, 
  FileWriteInput, 
  FileEditInput,
  AgentInput,
  BashInput,
  BashOutputInput,
  ExitPlanModeInput,
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

// Component for displaying Write tool content
export function WriteToolDisplay({ input }: { input: FileWriteInput }) {
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

// Component for displaying Agent tool content
export function AgentToolDisplay({ input }: { input: AgentInput }) {
  return (
    <div className="agent-tool-display">
      <div className="agent-tool-header">
        <h4>🤖 Agent Task</h4>
        <div className="agent-type">Agent: {input.subagent_type}</div>
        <div className="task-description">{input.description}</div>
      </div>
      <CollapsibleContent 
        content={input.prompt}
        className="agent-prompt"
        maxLines={15}
      />
    </div>
  );
}

// Component for displaying Bash tool content
export function BashToolDisplay({ input }: { input: BashInput }) {
  return (
    <div className="bash-tool-display">
      <div className="bash-tool-header">
        <h4>🖥️ Command Execution</h4>
        {input.description && <div className="bash-description">{input.description}</div>}
        <div className="bash-flags">
          {input.run_in_background && <span className="flag">🔄 Background</span>}
          {input.timeout && <span className="flag">⏱️ Timeout: {input.timeout}ms</span>}
        </div>
      </div>
      <CollapsibleContent 
        content={input.command}
        className="bash-command"
        maxLines={10}
        isCode={true}
      />
    </div>
  );
}

// Component for displaying BashOutput tool content
export function BashOutputToolDisplay({ input }: { input: BashOutputInput }) {
  return (
    <div className="bash-output-tool-display">
      <div className="bash-output-header">
        <h4>📄 Shell Output</h4>
        <div className="bash-id">Shell ID: {input.bash_id}</div>
        {input.filter && <div className="output-filter">Filter: {input.filter}</div>}
      </div>
    </div>
  );
}

// Component for displaying ExitPlanMode tool content
export function ExitPlanModeToolDisplay({ input }: { input: ExitPlanModeInput }) {
  return (
    <div className="exit-plan-mode-tool-display">
      <div className="exit-plan-mode-header">
        <h4>📋 Plan Review</h4>
      </div>
      <MarkdownContent 
        content={input.plan}
        className="plan-content"
      />
    </div>
  );
}

// Component for displaying FileMultiEdit tool content
export function FileMultiEditToolDisplay({ input }: { input: FileMultiEditInput }) {
  return (
    <div className="file-multi-edit-tool-display">
      <div className="file-multi-edit-header">
        <h4>✏️ Multi-Edit Operations</h4>
        <div className="file-path">📄 {input.file_path}</div>
        <div className="edit-count">{input.edits.length} operations</div>
      </div>
      <div className="edit-operations">
        {input.edits.map((edit, index) => (
          <div key={index} className="edit-operation">
            <div className="edit-header">
              <span className="edit-number">Edit #{index + 1}</span>
              {edit.replace_all && <span className="replace-all-flag">Replace All</span>}
            </div>
            <EditDiff toolInput={{
              file_path: input.file_path,
              old_string: edit.old_string,
              new_string: edit.new_string,
              replace_all: edit.replace_all
            } as FileEditInput} />
          </div>
        ))}
      </div>
    </div>
  );
}

// Component for displaying FileRead tool content
export function FileReadToolDisplay({ input }: { input: FileReadInput }) {
  return (
    <div className="file-read-tool-display">
      <div className="file-read-header">
        <h4>📖 File Reading</h4>
        <div className="file-path">📄 {input.file_path}</div>
        {(input.offset || input.limit) && (
          <div className="read-range">
            {input.offset && <span>From line {input.offset}</span>}
            {input.offset && input.limit && <span> • </span>}
            {input.limit && <span>Read {input.limit} lines</span>}
          </div>
        )}
      </div>
    </div>
  );
}

// Component for displaying Glob tool content
export function GlobToolDisplay({ input }: { input: GlobInput }) {
  return (
    <div className="glob-tool-display">
      <div className="glob-header">
        <h4>🔍 Pattern Search</h4>
        <div className="glob-pattern">Pattern: {input.pattern}</div>
        {input.path && <div className="search-path">Path: {input.path}</div>}
      </div>
    </div>
  );
}

// Component for displaying Grep tool content
export function GrepToolDisplay({ input }: { input: GrepInput }) {
  return (
    <div className="grep-tool-display">
      <div className="grep-header">
        <h4>🔎 Text Search</h4>
        <div className="grep-pattern">Pattern: {input.pattern}</div>
        {input.path && <div className="search-path">Path: {input.path}</div>}
        {input.glob && <div className="glob-filter">Files: {input.glob}</div>}
        <div className="grep-options">
          {input.output_mode && <span className="option">Mode: {input.output_mode}</span>}
          {input['-i'] && <span className="option">Case Insensitive</span>}
          {input.multiline && <span className="option">Multiline</span>}
          {input.type && <span className="option">Type: {input.type}</span>}
        </div>
      </div>
    </div>
  );
}

// Component for displaying KillShell tool content
export function KillShellToolDisplay({ input }: { input: KillShellInput }) {
  return (
    <div className="kill-shell-tool-display">
      <div className="kill-shell-header">
        <h4>⚠️ Terminate Shell</h4>
        <div className="shell-id">Shell ID: {input.shell_id}</div>
      </div>
    </div>
  );
}

// Component for displaying ListMcpResources tool content
export function ListMcpResourcesToolDisplay({ input }: { input: ListMcpResourcesInput }) {
  return (
    <div className="list-mcp-resources-tool-display">
      <div className="list-mcp-resources-header">
        <h4>📚 MCP Resources</h4>
        {input.server && <div className="server-filter">Server: {input.server}</div>}
      </div>
    </div>
  );
}

// Component for displaying Mcp tool content
export function McpToolDisplay({ input }: { input: McpInput }) {
  return (
    <div className="mcp-tool-display">
      <div className="mcp-header">
        <h4>🔌 MCP Operation</h4>
      </div>
      <CollapsibleContent 
        content={JSON.stringify(input, null, 2)}
        className="mcp-data"
        maxLines={15}
        isCode={true}
      />
    </div>
  );
}

// Component for displaying NotebookEdit tool content
export function NotebookEditToolDisplay({ input }: { input: NotebookEditInput }) {
  return (
    <div className="notebook-edit-tool-display">
      <div className="notebook-edit-header">
        <h4>📓 Notebook Editing</h4>
        <div className="notebook-path">📄 {input.notebook_path}</div>
        <div className="notebook-operation">
          {input.edit_mode || 'replace'} {input.cell_type && `${input.cell_type} cell`}
          {input.cell_id && ` (ID: ${input.cell_id})`}
        </div>
      </div>
      <CollapsibleContent 
        content={input.new_source}
        className="notebook-source"
        maxLines={15}
        isCode={input.cell_type === 'code'}
      />
    </div>
  );
}

// Component for displaying ReadMcpResource tool content
export function ReadMcpResourceToolDisplay({ input }: { input: ReadMcpResourceInput }) {
  return (
    <div className="read-mcp-resource-tool-display">
      <div className="read-mcp-resource-header">
        <h4>📖 MCP Resource</h4>
        <div className="mcp-server">Server: {input.server}</div>
        <div className="resource-uri">URI: {input.uri}</div>
      </div>
    </div>
  );
}

// Component for displaying WebFetch tool content
export function WebFetchToolDisplay({ input }: { input: WebFetchInput }) {
  return (
    <div className="web-fetch-tool-display">
      <div className="web-fetch-header">
        <h4>🌐 Web Fetch</h4>
        <div className="fetch-url">🔗 {input.url}</div>
      </div>
      <CollapsibleContent 
        content={input.prompt}
        className="fetch-prompt"
        maxLines={10}
      />
    </div>
  );
}

// Component for displaying WebSearch tool content
export function WebSearchToolDisplay({ input }: { input: WebSearchInput }) {
  return (
    <div className="web-search-tool-display">
      <div className="web-search-header">
        <h4>🔍 Web Search</h4>
        <div className="search-query">Query: {input.query}</div>
        {input.allowed_domains && input.allowed_domains.length > 0 && (
          <div className="domain-filters">
            <span>Allowed: {input.allowed_domains.join(', ')}</span>
          </div>
        )}
        {input.blocked_domains && input.blocked_domains.length > 0 && (
          <div className="domain-filters">
            <span>Blocked: {input.blocked_domains.join(', ')}</span>
          </div>
        )}
      </div>
    </div>
  );
}

interface MessageParserProps {
  data: unknown;
  timestamp?: number;
  showRawJson: boolean;
  messageSource: 'session' | 'websocket';
  onApprove?: (requestId: string, input: ToolInputSchemas, permissionUpdates?: PermissionUpdate[]) => void;
  onDeny?: (requestId: string) => void;
  onModeChange?: (mode: PermissionMode) => void;
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
  if (isSDKControlResponseMessage(message)) {
    return 'Control Response';
  }
  if (isSDKSummaryMessage(message)) {
    return 'Summary Generated';
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

export function MessageParser({ data, timestamp, showRawJson, onApprove, onDeny, onModeChange }: MessageParserProps) {
  const parsed = parseMessage(data);

  if (showRawJson) {
    return (
      <div className="message-item raw-json">
        <div className="message-header">
          <span className="message-type">Raw JSON</span>
          <MessageInfoButton timestamp={timestamp} messageType="Raw JSON" />
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
      return <FormattedClaudeMessage message={parsed.message} timestamp={timestamp} onApprove={onApprove} onDeny={onDeny} onModeChange={onModeChange} />;
    } catch (error) {
      console.error('Error rendering Claude message:', error, 'Message data:', parsed.message);
      return (
        <div className="message-error fallback">
          <div className="message-header">
            <span className="message-type">❌ Parse Error - {parsed.messageType}</span>
            <MessageInfoButton timestamp={timestamp} messageType={parsed.messageType} />
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
        <MessageInfoButton timestamp={timestamp} messageType={parsed.messageType} />
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

function FormattedClaudeMessage({ message, timestamp, onApprove, onDeny, onModeChange }: { 
  message: ExtendedSDKMessage; 
  timestamp?: number;
  onApprove?: (requestId: string, input: ToolInputSchemas, permissionUpdates?: PermissionUpdate[]) => void;
  onDeny?: (requestId: string) => void;
  onModeChange?: (mode: PermissionMode) => void;
}) {
  if (isSDKUserMessage(message)) {
    return (
      <div className="user-message">
        <div className="message-role">
          👤 User
          <MessageInfoButton timestamp={timestamp} messageType="User Message" />
        </div>
        <div className="message-content-blocks">
          {Array.isArray(message.message.content) ? (
            message.message.content.map((block: Record<string, unknown>, index: number) => (
              <div key={index} className={`content-block ${block.type || 'unknown'}`}>
                {block.type === 'text' && (
                  <MarkdownContent 
                    content={String(block.text)} 
                    className="text-content"
                  />
                )}
                {block.type === 'tool_result' && (
                  <div className="tool-result-simple">
                    <div className="tool-result-header-inline">
                      <span className="result-icon">✓</span>
                      <span className="result-label">Result</span>
                      {typeof block.tool_use_id === 'string' && (
                        <ToolInfoButton 
                          toolName="Tool Result" 
                          toolId={block.tool_use_id}
                        />
                      )}
                    </div>
                    <CollapsibleContent 
                      content={(() => {
                        // Handle different content structures
                        if (typeof block.content === 'string') {
                          return block.content;
                        }
                        // Handle array of content blocks
                        if (Array.isArray(block.content)) {
                          const textContent = block.content
                            .filter((item: unknown) => typeof item === 'object' && item !== null && 'type' in item && (item as {type: string}).type === 'text')
                            .map((item: unknown) => (item as {text: string}).text)
                            .join('\n');
                          if (textContent) {
                            return textContent;
                          }
                        }
                        // Fallback to JSON stringification
                        return JSON.stringify(block.content, null, 2);
                      })()}
                      className="tool-result-text-simple"
                      maxLines={15}
                      isCode={true}
                    />
                  </div>
                )}
                {block.type === 'tool_use' && (
                  <div className="tool-use-simple">
                    <div className="tool-header-inline">
                      <span className="tool-icon">🛠️</span>
                      <span className="tool-name-inline">{String(block.name)}</span>
                      <ToolInfoButton 
                        toolName={String(block.name)} 
                        toolId={String(block.id)}
                      />
                    </div>
                    {String(block.name) === 'TodoWrite' ? (
                      (() => {
                        const todos = parseTodosFromToolUse(block.input);
                        return todos ? (
                          <TodoList todos={todos} />
                        ) : (
                          <CollapsibleContent 
                            content={JSON.stringify(block.input, null, 2)}
                            className="tool-input-simple"
                            maxLines={10}
                            isCode={true}
                          />
                        );
                      })()
                    ) : String(block.name) === "Edit" ? (
                      <EditDiff toolInput={block.input as FileEditInput} />
                    ) : String(block.name) === "Write" ? (
                      <div className="tool-write-simple">
                        <div className="file-path-inline">📄 {(block.input as FileWriteInput).file_path}</div>
                        <CollapsibleContent 
                          content={(block.input as FileWriteInput).content}
                          className="file-content-simple"
                          maxLines={20}
                          isCode={true}
                        />
                      </div>
                    ) : String(block.name) === "ExitPlanMode" ? (
                      <div className="tool-exitplanmode-simple">
                        <div className="plan-label-inline">📋 Plan:</div>
                        <MarkdownContent 
                          content={(block.input as ExitPlanModeInput).plan}
                          className="plan-content-simple"
                        />
                      </div>
                    ) : (
                      <CollapsibleContent 
                        content={JSON.stringify(block.input, null, 2)}
                        className="tool-input-simple"
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
          <MessageInfoButton timestamp={timestamp} messageType="Assistant Message" />
        </div>
        <div className="message-content-blocks">
          {message.message.content.map((block: Record<string, unknown>, index: number) => (
            <div key={index} className={`content-block ${block.type}`}>
              {block.type === 'text' && (
                <MarkdownContent 
                  content={String(block.text)} 
                  className="text-content"
                />
              )}
              {block.type === 'tool_use' && (
                <div className="tool-use-simple">
                  <div className="tool-header-inline">
                    <span className="tool-icon">🛠️</span>
                    <span className="tool-name-inline">{String(block.name)}</span>
                    <ToolInfoButton 
                      toolName={String(block.name)} 
                      toolId={String(block.id)}
                    />
                  </div>
                  {String(block.name) === 'TodoWrite' ? (
                    (() => {
                      const todos = parseTodosFromToolUse(block.input);
                      return todos ? (
                        <TodoList todos={todos} />
                      ) : (
                        <CollapsibleContent 
                          content={JSON.stringify(block.input, null, 2)}
                          className="tool-input-simple"
                          maxLines={10}
                          isCode={true}
                        />
                      );
                    })()
                  ) : String(block.name) === "Edit" ? (
                    <EditDiff toolInput={block.input as FileEditInput} />
                  ) : String(block.name) === "Write" ? (
                    <div className="tool-write-simple">
                      <div className="file-path-inline">📄 {(block.input as FileWriteInput).file_path}</div>
                      <CollapsibleContent 
                        content={(block.input as FileWriteInput).content}
                        className="file-content-simple"
                        maxLines={20}
                        isCode={true}
                      />
                    </div>
                  ) : String(block.name) === "Task" ? (
                    <div className="tool-task-simple">
                      <div className="agent-info-inline">Agent: {(block.input as AgentInput).subagent_type} - {(block.input as AgentInput).description}</div>
                      <CollapsibleContent 
                        content={(block.input as AgentInput).prompt}
                        className="agent-prompt-simple"
                        maxLines={15}
                      />
                    </div>
                  ) : String(block.name) === "Bash" ? (
                    <div className="tool-bash-simple">
                      {(block.input as BashInput).description && <div className="bash-desc-inline">{(block.input as BashInput).description}</div>}
                      <CollapsibleContent 
                        content={(block.input as BashInput).command}
                        className="bash-command-simple"
                        maxLines={10}
                        isCode={true}
                      />
                    </div>
                  ) : String(block.name) === "Read" ? (
                    <div className="file-path-inline">📖 {(block.input as FileReadInput).file_path}</div>
                  ) : String(block.name) === "Glob" ? (
                    <div className="search-pattern-inline">🔍 Pattern: {(block.input as GlobInput).pattern}</div>
                  ) : String(block.name) === "Grep" ? (
                    <div className="search-pattern-inline">🔎 Search: {(block.input as GrepInput).pattern}</div>
                  ) : String(block.name) === "MultiEdit" ? (
                    <div className="tool-multiedit-simple">
                      <div className="file-path-inline">📄 {(block.input as FileMultiEditInput).file_path} ({(block.input as FileMultiEditInput).edits.length} edits)</div>
                      {(block.input as FileMultiEditInput).edits.map((edit, index) => (
                        <EditDiff key={index} toolInput={{
                          file_path: (block.input as FileMultiEditInput).file_path,
                          old_string: edit.old_string,
                          new_string: edit.new_string,
                          replace_all: edit.replace_all
                        } as FileEditInput} />
                      ))}
                    </div>
                  ) : String(block.name) === "WebFetch" ? (
                    <div className="web-url-inline">🌐 {(block.input as WebFetchInput).url}</div>
                  ) : String(block.name) === "WebSearch" ? (
                    <div className="search-query-inline">🔍 Query: {(block.input as WebSearchInput).query}</div>
                  ) : String(block.name) === "ExitPlanMode" ? (
                    <div className="tool-exitplanmode-simple">
                      <div className="plan-label-inline">📋 Plan:</div>
                      <MarkdownContent 
                        content={(block.input as ExitPlanModeInput).plan}
                        className="plan-content-simple"
                      />
                    </div>
                  ) : (
                    <CollapsibleContent 
                      content={JSON.stringify(block.input, null, 2)}
                      className="tool-input-simple"
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
    return <ControlRequestMessage message={message} timestamp={timestamp} onApprove={onApprove} onDeny={onDeny} onModeChange={onModeChange} />;
  }

  if (isSDKControlResponseMessage(message)) {
    const response = message.response;
    const isSuccess = response.subtype === 'success';
    
    // For success responses with mode changes, show minimal inline message
    if (isSuccess && response.response && 'mode' in response.response) {
      return (
        <div className={`control-response-message success`}>
          <span className="response-id">Mode: {(response.response as { mode?: string }).mode}</span>
        </div>
      );
    }
    
    // For other responses, show minimal info
    return (
      <div className={`control-response-message ${isSuccess ? 'success' : 'error'}`}>
        <span className="response-id">
          {isSuccess ? '✓' : '✗'}
        </span>
        {response.error && (
          <span className="control-response-content">
            <span className="control-response-details">{response.error}</span>
          </span>
        )}
      </div>
    );
  }

  if (isSDKSummaryMessage(message)) {
    return (
      <div className="summary-message">
        <span className="summary-text">📝 Generated summary for message {message.leafUuid}: {message.summary}</span>
      </div>
    );
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

