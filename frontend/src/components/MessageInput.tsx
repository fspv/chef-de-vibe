import { useState } from 'react';
import { v4 as uuidv4 } from 'uuid';

interface MessageInputProps {
  onSendMessage: (message: string) => void;
  disabled: boolean;
  debugMode: boolean;
}

export function MessageInput({ onSendMessage, disabled, debugMode }: MessageInputProps) {
  const [input, setInput] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submitMessage();
  };

  const submitMessage = () => {
    if (input.trim() && !disabled) {
      if (debugMode) {
        // Raw JSON mode
        try {
          JSON.parse(input);
          onSendMessage(input);
          setInput('');
        } catch {
          alert('Invalid JSON format. Please check your input.');
        }
      } else {
        // Normal text mode - format as minimal Claude message
        const message = {
          type: 'user',
          message: {
            role: 'user',
            content: input.trim()
          },
          parent_tool_use_id: null,
          uuid: uuidv4(),
          session_id: '' // This will be filled by the backend
        };
        onSendMessage(JSON.stringify(message));
        setInput('');
      }
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submitMessage();
    }
  };

  return (
    <form onSubmit={handleSubmit} className="message-input">
      <div className="input-area">
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={
            disabled 
              ? "Connecting..." 
              : debugMode
                ? "Enter raw JSON message (e.g., {\"role\": \"user\", \"content\": \"Hello\"})"
                : "Type your message... (Ctrl/Cmd+Enter to send)"
          }
          disabled={disabled}
          rows={3}
        />
        <button type="submit" disabled={disabled || !input.trim()}>
          Send
        </button>
      </div>
    </form>
  );
}