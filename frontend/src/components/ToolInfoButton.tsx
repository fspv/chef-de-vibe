import { useState } from 'react';

interface ToolInfoButtonProps {
  toolName: string;
  toolId: string;
  additionalInfo?: Record<string, unknown>;
}

export function ToolInfoButton({ toolName, toolId, additionalInfo }: ToolInfoButtonProps) {
  const [showInfo, setShowInfo] = useState(false);

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    setShowInfo(!showInfo);
  };

  const handleClose = () => {
    setShowInfo(false);
  };

  return (
    <div className="tool-info-button-container">
      <button 
        className="tool-info-button"
        onClick={handleClick}
        title="Show tool details"
        aria-label="Show tool information"
      >
        ⓘ
      </button>
      
      {showInfo && (
        <>
          <div className="tool-info-overlay" onClick={handleClose} />
          <div className="tool-info-popup">
            <div className="tool-info-header">
              <h4>Tool Details</h4>
              <button className="tool-info-close" onClick={handleClose}>×</button>
            </div>
            <div className="tool-info-content">
              <div className="info-row">
                <span className="info-label">Tool:</span>
                <span className="info-value">{toolName}</span>
              </div>
              <div className="info-row">
                <span className="info-label">ID:</span>
                <span className="info-value tool-id-full">{toolId}</span>
              </div>
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