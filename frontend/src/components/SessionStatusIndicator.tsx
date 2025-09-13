import { useState, useEffect } from 'react';

interface SessionStatusIndicatorProps {
  isActive: boolean;
  isMainConnected: boolean;
  isApprovalConnected: boolean;
  hasApprovalRequests: boolean;
  approvalRequestCount: number;
  sessionId: string;
  workingDirectory: string;
  debugMode: boolean;
  onDebugModeChange: (value: boolean) => void;
}

export function SessionStatusIndicator({
  isActive,
  isMainConnected,
  isApprovalConnected,
  hasApprovalRequests,
  approvalRequestCount,
  sessionId,
  workingDirectory,
  debugMode,
  onDebugModeChange
}: SessionStatusIndicatorProps) {
  const [showDetails, setShowDetails] = useState(false);

  const getStatusColor = () => {
    if (!isActive) return 'red';
    if (!isMainConnected || !isApprovalConnected || hasApprovalRequests) return 'yellow';
    return 'green';
  };

  const getStatusText = () => {
    if (!isActive) return 'Inactive';
    if (!isMainConnected || !isApprovalConnected) return 'Connecting...';
    if (hasApprovalRequests) return `${approvalRequestCount} pending`;
    return 'Active';
  };

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  // Handle escape key to close details popup
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && showDetails) {
        setShowDetails(false);
      }
    };

    if (showDetails) {
      window.addEventListener('keydown', handleKeyDown);
      return () => window.removeEventListener('keydown', handleKeyDown);
    }
  }, [showDetails]);

  const statusColor = getStatusColor();

  return (
    <>
      <button
        className={`session-status-indicator status-${statusColor}`}
        onClick={() => setShowDetails(!showDetails)}
        title={getStatusText()}
      >
        <span className="status-dot"></span>
      </button>

      {showDetails && (
        <div className="session-details-overlay" onClick={() => setShowDetails(false)}>
          <div className="session-details-popup" onClick={(e) => e.stopPropagation()}>
            <div className="session-details-header">
              <h3>Session Details</h3>
              <button className="close-button" onClick={() => setShowDetails(false)}>
                ×
              </button>
            </div>

            <div className="session-details-content">
              <div className="session-detail-row">
                <span className="detail-label">Session ID:</span>
                <div className="detail-value-with-copy">
                  <code className="detail-value">{sessionId}</code>
                  <button 
                    className="copy-button-small" 
                    onClick={() => handleCopy(sessionId)}
                    title="Copy session ID"
                  >
                    📋
                  </button>
                </div>
              </div>

              <div className="session-detail-row">
                <span className="detail-label">Working Directory:</span>
                <div className="detail-value-with-copy">
                  <code className="detail-value">{workingDirectory}</code>
                  <button 
                    className="copy-button-small" 
                    onClick={() => handleCopy(workingDirectory)}
                    title="Copy working directory"
                  >
                    📋
                  </button>
                </div>
              </div>

              <div className="connection-details">
                <h4>Connection Status</h4>
                <div className="connection-status-list">
                  <div className={`connection-item ${isMainConnected ? 'connected' : 'disconnected'}`}>
                    <span className="connection-dot"></span>
                    <span>Main WebSocket</span>
                  </div>
                  <div className={`connection-item ${isApprovalConnected ? 'connected' : 'disconnected'}`}>
                    <span className="connection-dot"></span>
                    <span>Approvals WebSocket</span>
                  </div>
                </div>
              </div>

              <div className="debug-mode-section">
                <label className="debug-mode-toggle">
                  <input
                    type="checkbox"
                    checked={debugMode}
                    onChange={(e) => onDebugModeChange(e.target.checked)}
                  />
                  <span>Debug Mode</span>
                </label>
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}