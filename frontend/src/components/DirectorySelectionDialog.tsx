import { useState } from 'react';
import { DirectoryPicker } from './DirectoryPicker';

interface DirectorySelectionDialogProps {
  onSelectDirectory: (directory: string) => void;
  onCancel: () => void;
}

export function DirectorySelectionDialog({ onSelectDirectory, onCancel }: DirectorySelectionDialogProps) {
  const [selectedDirectory, setSelectedDirectory] = useState('/tmp');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (selectedDirectory.trim()) {
      onSelectDirectory(selectedDirectory.trim());
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onCancel();
    }
  };

  return (
    <div className="directory-selection-overlay" onClick={onCancel}>
      <div 
        className="directory-selection-dialog" 
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="directory-dialog-header">
          <h3>Select Working Directory</h3>
          <button 
            className="dialog-close-button" 
            onClick={onCancel}
            title="Cancel (Esc)"
          >
            ✕
          </button>
        </div>
        
        <form onSubmit={handleSubmit} className="directory-dialog-content">
          <div className="directory-dialog-description">
            <p>Choose the working directory for your new chat session. This will be where Claude can read and write files.</p>
          </div>
          
          <div className="directory-input-container">
            <label htmlFor="directory-input">Working Directory:</label>
            <DirectoryPicker
              value={selectedDirectory}
              onChange={setSelectedDirectory}
              placeholder="Type or select a directory path..."
              className="directory-dialog-picker"
            />
          </div>
          
          <div className="directory-dialog-actions">
            <button 
              type="button" 
              className="cancel-button"
              onClick={onCancel}
            >
              Cancel
            </button>
            <button 
              type="submit" 
              className="confirm-button"
              disabled={!selectedDirectory.trim()}
            >
              Start Chat
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}