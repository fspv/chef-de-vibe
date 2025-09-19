import { useState } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { DirectoryPicker } from './DirectoryPicker';

interface NewChatDialogProps {
  onStartChat: (directory: string, firstMessage: string) => Promise<void>;
  onCancel: () => void;
}

export function NewChatDialog({ onStartChat, onCancel }: NewChatDialogProps) {
  const [selectedDirectory, setSelectedDirectory] = useState('');
  const [firstMessage, setFirstMessage] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (selectedDirectory.trim() && firstMessage.trim()) {
      setIsLoading(true);
      setError(null);
      
      // Format the first message exactly like MessageInput does
      const message = {
        type: 'user',
        message: {
          role: 'user',
          content: firstMessage.trim()
        },
        parent_tool_use_id: null,
        uuid: uuidv4(),
        session_id: '' // This will be filled by the backend
      };
      
      try {
        await onStartChat(selectedDirectory.trim(), JSON.stringify(message));
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to create session');
        setIsLoading(false);
      }
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onCancel();
    }
  };

  return (
    <div className="new-chat-overlay" onClick={onCancel}>
      <div 
        className="new-chat-dialog" 
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="new-chat-header">
          <h3>Start New Chat</h3>
          <button 
            className="dialog-close-button" 
            onClick={onCancel}
            title="Cancel (Esc)"
          >
            ✕
          </button>
        </div>
        
        <form onSubmit={handleSubmit} className="new-chat-content">
          <div className="new-chat-description">
            <p>Choose the working directory and your first message to start a new chat session with Claude.</p>
            {error && (
              <div className="error-message" style={{ color: 'red', marginTop: '10px', padding: '10px', backgroundColor: '#fee', borderRadius: '4px' }}>
                {error}
              </div>
            )}
          </div>
          
          <div className="directory-input-container">
            <label htmlFor="directory-input">Working Directory:</label>
            <DirectoryPicker
              value={selectedDirectory}
              onChange={setSelectedDirectory}
              placeholder="Type or select a directory path..."
              className="new-chat-picker"
            />
          </div>
          
          <div className="message-input-container">
            <label htmlFor="message-input">Your Message:</label>
            <textarea
              id="message-input"
              value={firstMessage}
              onChange={(e) => setFirstMessage(e.target.value)}
              placeholder='Enter your first message (e.g., "Hello Claude, help me with my project")'
              className="first-message-input"
              rows={4}
              required
            />
          </div>
          
          <div className="new-chat-actions">
            <button 
              type="button" 
              className="cancel-button"
              onClick={onCancel}
              disabled={isLoading}
            >
              Cancel
            </button>
            <button 
              type="submit" 
              className="confirm-button"
              disabled={!selectedDirectory.trim() || !firstMessage.trim() || isLoading}
            >
              {isLoading ? 'Starting...' : 'Start Chat'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}