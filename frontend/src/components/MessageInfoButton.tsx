import { useState } from 'react';

interface MessageInfoButtonProps {
  timestamp?: number;
  messageType?: string;
  additionalInfo?: Record<string, any>;
}

export function MessageInfoButton({ timestamp, messageType, additionalInfo }: MessageInfoButtonProps) {
  const [showInfo, setShowInfo] = useState(false);

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    setShowInfo(!showInfo);
  };

  const handleClose = () => {
    setShowInfo(false);
  };

  return (
    <div className="message-info-button-container">
      <button 
        className="message-info-button"
        onClick={handleClick}
        title="Show message details"
        aria-label="Show message information"
      >
        ⓘ
      </button>
      
      {showInfo && (
        <>
          <div className="message-info-overlay" onClick={handleClose} />
          <div className="message-info-popup">
            <div className="message-info-header">
              <h4>Message Details</h4>
              <button className="message-info-close" onClick={handleClose}>×</button>
            </div>
            <div className="message-info-content">
              {timestamp && (
                <div className="info-row">
                  <span className="info-label">Time:</span>
                  <span className="info-value">{new Date(timestamp).toLocaleTimeString()}</span>
                </div>
              )}
              {timestamp && (
                <div className="info-row">
                  <span className="info-label">Date:</span>
                  <span className="info-value">{new Date(timestamp).toLocaleDateString()}</span>
                </div>
              )}
              {messageType && (
                <div className="info-row">
                  <span className="info-label">Type:</span>
                  <span className="info-value">{messageType}</span>
                </div>
              )}
              {additionalInfo && Object.entries(additionalInfo).map(([key, value]) => (
                <div key={key} className="info-row">
                  <span className="info-label">{key}:</span>
                  <span className="info-value">{String(value)}</span>
                </div>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
}