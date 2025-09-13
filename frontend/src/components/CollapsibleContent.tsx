import { useState } from 'react';

interface CollapsibleContentProps {
  content: string;
  maxLines?: number;
  className?: string;
  isCode?: boolean;
}

export function CollapsibleContent({ 
  content, 
  maxLines = 10, 
  className = '', 
  isCode = false 
}: CollapsibleContentProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(content);
    } catch {
      // Fallback for older browsers
      const textArea = document.createElement('textarea');
      textArea.value = content;
      document.body.appendChild(textArea);
      textArea.select();
      document.execCommand('copy');
      document.body.removeChild(textArea);
    }
  };
  
  const lines = content.split('\n');
  const needsCollapse = lines.length > maxLines;
  
  if (!needsCollapse) {
    return (
      <div className={`${className}`} style={{ 
        position: 'relative'
      }}>
        {isCode && (
          <div style={{
            position: 'absolute',
            top: '8px',
            right: '8px',
            zIndex: 1
          }}>
            <button
              type="button"
              onClick={copyToClipboard}
              style={{
                padding: '4px 8px',
                fontSize: '12px',
                border: '1px solid #ccc',
                borderRadius: '4px',
                background: '#f9f9f9',
                cursor: 'pointer'
              }}
              title="Copy to clipboard"
            >
              📋
            </button>
          </div>
        )}
        {isCode ? (
          <pre style={{ 
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
            overflowWrap: 'break-word',
            width: '100%',
            maxWidth: '100%',
            margin: 0,
            paddingTop: '32px', // Space for copy button
            boxSizing: 'border-box'
          }}>
            {content}
          </pre>
        ) : (
          <div style={{ 
            whiteSpace: 'pre-wrap',
            maxWidth: '100%',
            overflowWrap: 'break-word',
            wordWrap: 'break-word'
          }}>
            {content}
          </div>
        )}
      </div>
    );
  }
  
  const displayContent = isExpanded 
    ? content 
    : lines.slice(0, maxLines).join('\n');
  
  return (
    <div className={`collapsible-content ${className}`} style={{ 
      position: 'relative'
    }}>
      {isCode && (
        <div style={{
          position: 'absolute',
          top: '8px',
          right: '8px',
          zIndex: 1
        }}>
          <button
            type="button"
            onClick={copyToClipboard}
            style={{
              padding: '4px 8px',
              fontSize: '12px',
              border: '1px solid #ccc',
              borderRadius: '4px',
              background: '#f9f9f9',
              cursor: 'pointer'
            }}
            title="Copy to clipboard"
          >
            📋
          </button>
        </div>
      )}
      {isCode ? (
        <pre style={{ 
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
          overflowWrap: 'break-word',
          width: '100%',
          maxWidth: '100%',
          margin: 0,
          paddingTop: '32px', // Space for copy button
          boxSizing: 'border-box'
        }}>
          {displayContent}
        </pre>
      ) : (
        <div style={{ 
          whiteSpace: 'pre-wrap',
          maxWidth: '100%',
          overflowWrap: 'break-word',
          wordWrap: 'break-word'
        }}>
          {displayContent}
        </div>
      )}
      {!isExpanded && (
        <div className="content-fade">
          <div className="fade-gradient"></div>
        </div>
      )}
      <button 
        className="collapse-toggle"
        onClick={() => setIsExpanded(!isExpanded)}
        type="button"
      >
        {isExpanded 
          ? `▲ Show Less (${lines.length - maxLines} lines hidden)` 
          : `▼ Show More (${lines.length - maxLines} more lines)`
        }
      </button>
    </div>
  );
}