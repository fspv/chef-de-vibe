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
  // Create unified diff
  const diffText = createUnifiedDiff(oldString, newString, fileName);
  
  // Parse the diff
  const files = parseDiff(diffText);
  
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
    <div className="diff-viewer-container">
      <div className="diff-file-header">
        <span className="diff-file-name">{fileName}</span>
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
  );
}

// Component specifically for Edit tool usage
export function EditDiff({ toolInput }: { toolInput: FileEditInput }) {
  const { file_path, old_string, new_string } = toolInput;
  
  return (
    <div className="edit-diff">
      <div className="edit-diff-header">
        <h4>📝 File Changes</h4>
        {toolInput.replace_all && (
          <span className="replace-all-indicator">Replace All</span>
        )}
      </div>
      <DiffViewer 
        oldString={old_string}
        newString={new_string}
        fileName={file_path}
      />
    </div>
  );
}
