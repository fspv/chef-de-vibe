import { useState, useEffect, useCallback } from 'react';
import { parseDiff, Diff, Hunk } from 'react-diff-view';
import 'react-diff-view/style/index.css';
import type { FileEditInput } from '@anthropic-ai/claude-code/sdk-tools';

interface DiffViewerProps {
  oldString: string;
  newString: string;
  fileName?: string;
}

// Create a unified diff from old and new strings
function createUnifiedDiff(oldString: string, newString: string, fileName = 'file'): string {
  const oldLines = oldString.split('\n');
  const newLines = newString.split('\n');
  
  const diffHeader = `--- a/${fileName}\n+++ b/${fileName}\n`;
  const hunkHeader = `@@ -1,${oldLines.length} +1,${newLines.length} @@\n`;
  
  let diff = diffHeader + hunkHeader;
  
  // Simple line-by-line diff (this is a basic implementation)
  const maxLines = Math.max(oldLines.length, newLines.length);
  
  for (let i = 0; i < maxLines; i++) {
    const oldLine = i < oldLines.length ? oldLines[i] : undefined;
    const newLine = i < newLines.length ? newLines[i] : undefined;
    
    if (oldLine === newLine) {
      if (oldLine !== undefined) {
        diff += ` ${oldLine}\n`;
      }
    } else {
      if (oldLine !== undefined) {
        diff += `-${oldLine}\n`;
      }
      if (newLine !== undefined) {
        diff += `+${newLine}\n`;
      }
    }
  }
  
  return diff;
}

// Note: Syntax highlighting will be handled by the library's default behavior

export function DiffViewer({ oldString, newString, fileName = 'file' }: DiffViewerProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);

  // Create unified diff
  const diffText = createUnifiedDiff(oldString, newString, fileName);
  
  // Parse the diff
  const files = parseDiff(diffText);
  
  // Handle ESC key to exit fullscreen
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isFullscreen) {
        setIsFullscreen(false);
      }
    };

    if (isFullscreen) {
      document.addEventListener('keydown', handleEsc);
      // Prevent body scroll when fullscreen
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }

    return () => {
      document.removeEventListener('keydown', handleEsc);
      document.body.style.overflow = '';
    };
  }, [isFullscreen]);

  const toggleFullscreen = useCallback(() => {
    setIsFullscreen(!isFullscreen);
  }, [isFullscreen]);
  
  if (files.length === 0) {
    return (
      <div className="diff-viewer-empty">
        <p>No differences detected</p>
      </div>
    );
  }
  
  const file = files[0];
  
  // Let the library handle tokenization and rendering - just use the hunks as-is
  const hunks = file.hunks;
  
  return (
    <>
      <div className={`diff-viewer-container ${isFullscreen ? 'diff-fullscreen' : ''}`}>
        <div className="diff-file-header">
          <span className="diff-file-name">{fileName}</span>
          <button 
            className="diff-fullscreen-btn"
            onClick={toggleFullscreen}
            title={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
          >
            {isFullscreen ? '✕' : '⛶'}
          </button>
        </div>
        <div className="diff-content">
          <Diff 
            viewType="unified" 
            diffType={file.type}
            hunks={hunks}
          >
            {(hunks) => hunks.map((hunk) => (
              <Hunk key={hunk.content} hunk={hunk} />
            ))}
          </Diff>
        </div>
      </div>
      {isFullscreen && <div className="diff-fullscreen-backdrop" onClick={toggleFullscreen} />}
    </>
  );
}

// Component specifically for Edit tool usage
export function EditDiff({ toolInput }: { toolInput: FileEditInput }) {
  const { file_path, old_string, new_string } = toolInput;
  const [isFullscreen, setIsFullscreen] = useState(false);

  // Handle ESC key to exit fullscreen
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isFullscreen) {
        setIsFullscreen(false);
      }
    };

    if (isFullscreen) {
      document.addEventListener('keydown', handleEsc);
      // Prevent body scroll when fullscreen
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }

    return () => {
      document.removeEventListener('keydown', handleEsc);
      document.body.style.overflow = '';
    };
  }, [isFullscreen]);

  const toggleFullscreen = useCallback(() => {
    setIsFullscreen(!isFullscreen);
  }, [isFullscreen]);
  
  return (
    <>
      <div className={`edit-diff-simple ${isFullscreen ? 'diff-fullscreen' : ''}`}>
        <div className="edit-diff-header-simple">
          <span className="edit-file-path">{file_path}</span>
          {toolInput.replace_all && (
            <span className="replace-all-badge">Replace All</span>
          )}
          <button 
            className="diff-fullscreen-btn"
            onClick={toggleFullscreen}
            title={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
          >
            {isFullscreen ? '✕' : '⛶'}
          </button>
        </div>
        <div className="diff-content-simple">
          <Diff 
            viewType="unified" 
            diffType="modify"
            hunks={parseDiff(createUnifiedDiff(old_string, new_string, file_path))[0]?.hunks || []}
          >
            {(hunks) => hunks.map((hunk) => (
              <Hunk key={hunk.content} hunk={hunk} />
            ))}
          </Diff>
        </div>
      </div>
      {isFullscreen && <div className="diff-fullscreen-backdrop" onClick={toggleFullscreen} />}
    </>
  );
}
