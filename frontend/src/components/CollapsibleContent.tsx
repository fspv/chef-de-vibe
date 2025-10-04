import { useState, useEffect, useCallback } from 'react';

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
      <>
        <div className={`${className} ${isFullscreen ? 'content-fullscreen' : ''}`} style={{ 
          position: 'relative'
        }}>
          {isCode && (
            <div style={{
              position: isFullscreen ? 'sticky' : 'absolute',
              top: '8px',
              right: '8px',
              zIndex: 1,
              display: 'flex',
              gap: '4px'
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
              <button
                type="button"
                onClick={toggleFullscreen}
                className="content-fullscreen-btn"
                title={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
              >
                {isFullscreen ? '✕' : '⛶'}
              </button>
            </div>
          )}
          <div className={isFullscreen ? 'content-fullscreen-body' : ''}>
            {isCode ? (
              <pre style={{ 
                whiteSpace: isFullscreen ? 'pre' : 'pre-wrap',
                wordBreak: isFullscreen ? 'normal' : 'break-word',
                overflowWrap: isFullscreen ? 'normal' : 'break-word',
                width: '100%',
                maxWidth: '100%',
                margin: 0,
                paddingTop: '40px', // Space for buttons
                boxSizing: 'border-box'
              }}>
                {content}
              </pre>
            ) : (
              <div style={{ 
                whiteSpace: isFullscreen ? 'pre' : 'pre-wrap',
                maxWidth: '100%',
                overflowWrap: isFullscreen ? 'normal' : 'break-word',
                wordWrap: isFullscreen ? 'normal' : 'break-word'
              }}>
                {content}
              </div>
            )}
          </div>
        </div>
        {isFullscreen && <div className="content-fullscreen-backdrop" onClick={toggleFullscreen} />}
      </>
    );
  }
  
  const displayContent = isExpanded 
    ? content 
    : lines.slice(0, maxLines).join('\n');
  
  return (
    <>
      <div className={`collapsible-content ${className} ${isFullscreen ? 'content-fullscreen' : ''}`} style={{ 
        position: 'relative'
      }}>
        {isCode && (
          <div style={{
            position: isFullscreen ? 'sticky' : 'absolute',
            top: '8px',
            right: '8px',
            zIndex: 1,
            display: 'flex',
            gap: '4px'
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
            <button
              type="button"
              onClick={toggleFullscreen}
              className="content-fullscreen-btn"
              title={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
            >
              {isFullscreen ? '✕' : '⛶'}
            </button>
          </div>
        )}
        <div className={isFullscreen ? 'content-fullscreen-body' : ''}>
          {isCode ? (
            <pre style={{ 
              whiteSpace: isFullscreen ? 'pre' : 'pre-wrap',
              wordBreak: isFullscreen ? 'normal' : 'break-word',
              overflowWrap: isFullscreen ? 'normal' : 'break-word',
              width: '100%',
              maxWidth: '100%',
              margin: 0,
              paddingTop: '40px', // Space for buttons
              boxSizing: 'border-box'
            }}>
              {isFullscreen ? content : displayContent}
            </pre>
          ) : (
            <div style={{ 
              whiteSpace: isFullscreen ? 'pre' : 'pre-wrap',
              maxWidth: '100%',
              overflowWrap: isFullscreen ? 'normal' : 'break-word',
              wordWrap: isFullscreen ? 'normal' : 'break-word'
            }}>
              {isFullscreen ? content : displayContent}
            </div>
          )}
        </div>
        {!isExpanded && !isFullscreen && (
          <div className="content-fade">
            <div className="fade-gradient"></div>
          </div>
        )}
        {!isFullscreen && (
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
        )}
      </div>
      {isFullscreen && <div className="content-fullscreen-backdrop" onClick={toggleFullscreen} />}
    </>
  );
}