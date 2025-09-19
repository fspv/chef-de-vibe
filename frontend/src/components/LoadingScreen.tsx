import { useState, useEffect } from 'react';
import './LoadingScreen.css';

export interface LogEntry {
  timestamp: number;
  message: string;
  type: 'info' | 'success' | 'error' | 'warning';
}

interface LoadingScreenProps {
  sessionId: string | null;
  operation: 'creating' | 'resuming' | 'loading';
  logs: LogEntry[];
}

export function LoadingScreen({ sessionId, operation, logs }: LoadingScreenProps) {
  const [dots, setDots] = useState('');
  
  useEffect(() => {
    const interval = setInterval(() => {
      setDots(prev => prev.length >= 3 ? '' : prev + '.');
    }, 500);
    
    return () => clearInterval(interval);
  }, []);
  
  const getOperationTitle = () => {
    switch (operation) {
      case 'creating':
        return 'Creating New Session';
      case 'resuming':
        return 'Resuming Session';
      case 'loading':
        return 'Loading Session';
      default:
        return 'Loading';
    }
  };
  
  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', { 
      hour12: false, 
      hour: '2-digit', 
      minute: '2-digit', 
      second: '2-digit',
      fractionalSecondDigits: 3
    });
  };
  
  return (
    <div className="loading-screen">
      <div className="loading-container">
        <div className="loading-header">
          <div className="loading-spinner"></div>
          <h2>{getOperationTitle()}{dots}</h2>
          {sessionId && (
            <p className="session-id">Session ID: {sessionId}</p>
          )}
        </div>
        
        <div className="loading-logs">
          <div className="log-entries">
            {logs.map((log, index) => (
              <div key={index} className={`log-entry log-${log.type}`}>
                <span className="log-timestamp">[{formatTimestamp(log.timestamp)}]</span>
                <span className="log-message">{log.message}</span>
              </div>
            ))}
            {logs.length === 0 && (
              <div className="log-entry log-info">
                <span className="log-timestamp">[{formatTimestamp(Date.now())}]</span>
                <span className="log-message">Initializing{dots}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}