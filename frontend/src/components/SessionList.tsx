import { useMemo, useState, useEffect } from 'react';
import { useSessions } from '../hooks/useApi';
import type { Session } from '../types/api';

interface SessionListProps {
  selectedSessionId: string | null;
  onSessionSelect: (sessionId: string) => void;
  onNewChat: () => void;
  directoryPopup?: string | null;
  onDirectoryPathClick?: (directory: string | null) => void;
}

interface SessionGroup {
  directory: string;
  sessions: Session[];
  latestMessageDate: string | null;
}

const COLLAPSED_DIRS_KEY = 'chef-de-vibe-collapsed-dirs';
const SESSIONS_PER_PAGE = 5;

export function SessionList({ selectedSessionId, onSessionSelect, onNewChat, directoryPopup: externalDirectoryPopup, onDirectoryPathClick }: SessionListProps) {
  const { sessions, loading, error, refetch } = useSessions();
  const [collapsedDirs, setCollapsedDirs] = useState<Set<string>>(new Set());
  const [showMoreCounts, setShowMoreCounts] = useState<Record<string, number>>({});
  const [internalDirectoryPopup, setInternalDirectoryPopup] = useState<string | null>(null);
  const [copySuccess, setCopySuccess] = useState<string | null>(null);
  
  // Use external popup state if provided, otherwise use internal
  const directoryPopup = externalDirectoryPopup !== undefined ? externalDirectoryPopup : internalDirectoryPopup;
  const setDirectoryPopup = onDirectoryPathClick || setInternalDirectoryPopup;

  // Load collapsed state from localStorage on mount
  useEffect(() => {
    const stored = localStorage.getItem(COLLAPSED_DIRS_KEY);
    if (stored) {
      try {
        const parsed = JSON.parse(stored);
        setCollapsedDirs(new Set(parsed));
      } catch (e) {
        console.error('Failed to parse collapsed dirs from localStorage:', e);
      }
    }
  }, []);

  // Save collapsed state to localStorage whenever it changes
  useEffect(() => {
    localStorage.setItem(COLLAPSED_DIRS_KEY, JSON.stringify(Array.from(collapsedDirs)));
  }, [collapsedDirs]);

  // Group sessions by working directory and sort
  const groupedSessions = useMemo(() => {
    if (!sessions) return [];

    // Group sessions by working directory
    const groups = sessions.reduce<Record<string, Session[]>>((acc, session) => {
      const dir = session.working_directory;
      if (!acc[dir]) {
        acc[dir] = [];
      }
      acc[dir].push(session);
      return acc;
    }, {});

    // Convert to array and calculate latest message date for each group
    const groupArray: SessionGroup[] = Object.entries(groups).map(([directory, groupSessions]) => {
      // Sort sessions within group by latest_message_date (most recent first)
      const sortedSessions = [...groupSessions].sort((a, b) => {
        const dateA = a.latest_message_date || a.earliest_message_date || '';
        const dateB = b.latest_message_date || b.earliest_message_date || '';
        return dateB.localeCompare(dateA);
      });

      // Find the most recent message date in this group
      const latestDate = sortedSessions.reduce<string | null>((latest, session) => {
        const sessionDate = session.latest_message_date || session.earliest_message_date || null;
        if (!sessionDate) return latest;
        if (!latest) return sessionDate;
        return sessionDate > latest ? sessionDate : latest;
      }, null);

      return {
        directory,
        sessions: sortedSessions,
        latestMessageDate: latestDate,
      };
    });

    // Sort groups by latest message date (most recent first)
    return groupArray.sort((a, b) => {
      if (!a.latestMessageDate && !b.latestMessageDate) return 0;
      if (!a.latestMessageDate) return 1;
      if (!b.latestMessageDate) return -1;
      return b.latestMessageDate.localeCompare(a.latestMessageDate);
    });
  }, [sessions]);

  const toggleDirectory = (directory: string) => {
    setCollapsedDirs(prev => {
      const next = new Set(prev);
      if (next.has(directory)) {
        next.delete(directory);
      } else {
        next.add(directory);
      }
      return next;
    });
  };

  const showMore = (directory: string) => {
    setShowMoreCounts(prev => ({
      ...prev,
      [directory]: (prev[directory] || 0) + SESSIONS_PER_PAGE
    }));
  };

  const getVisibleSessions = (group: SessionGroup) => {
    const additionalCount = showMoreCounts[group.directory] || 0;
    const totalVisible = SESSIONS_PER_PAGE + additionalCount;
    return {
      visibleSessions: group.sessions.slice(0, totalVisible),
      hasMore: group.sessions.length > totalVisible
    };
  };

  const handleDirectoryHeaderClick = (e: React.MouseEvent, directory: string) => {
    const target = e.target as HTMLElement;
    
    // Check if click was on the directory path specifically
    if (target.classList.contains('directory-path') || target.closest('.directory-path')) {
      e.stopPropagation();
      setDirectoryPopup(directory);
    } else {
      // Otherwise, toggle the directory
      toggleDirectory(directory);
    }
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopySuccess(text);
      setTimeout(() => setCopySuccess(null), 2000);
    } catch (err) {
      console.error('Failed to copy: ', err);
    }
  };

  const closePopup = () => {
    if (onDirectoryPathClick) {
      onDirectoryPathClick(null);
    } else {
      setInternalDirectoryPopup(null);
    }
    setCopySuccess(null);
  };

  if (loading) {
    return (
      <div className="session-list">
        <div className="session-list-header">
          <h2>Sessions</h2>
          <button onClick={onNewChat}>New Chat</button>
        </div>
        <div className="loading">Loading sessions...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="session-list">
        <div className="session-list-header">
          <h2>Sessions</h2>
          <button onClick={onNewChat}>New Chat</button>
        </div>
        <div className="error">
          <p>{error}</p>
          <button onClick={refetch}>Retry</button>
        </div>
      </div>
    );
  }

  const formatDate = (dateStr?: string) => {
    if (!dateStr) return '';
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    
    if (diffDays === 0) {
      // Today - show time
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } else if (diffDays === 1) {
      return 'Yesterday';
    } else if (diffDays < 7) {
      return date.toLocaleDateString([], { weekday: 'short' });
    } else {
      return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
    }
  };

  const truncateSummary = (text: string, maxLength: number = 60): string => {
    if (!text) return text;
    // Remove line breaks and normalize whitespace
    const normalized = text.replace(/[\r\n]+/g, ' ').replace(/\s+/g, ' ').trim();
    if (normalized.length <= maxLength) return normalized;
    return normalized.slice(0, maxLength) + '...';
  };

  return (
    <>
      <div className="session-list">
        <div className="session-list-header">
          <h2>Sessions</h2>
          <button onClick={onNewChat}>New Chat</button>
        </div>
        <div className="session-groups">
          {groupedSessions.length === 0 ? (
            <div className="empty-state">
              <p>No sessions found</p>
              <button onClick={onNewChat}>Start your first chat</button>
            </div>
          ) : (
            groupedSessions.map((group) => {
              const isCollapsed = collapsedDirs.has(group.directory);
              const hasSelectedSession = group.sessions.some(s => s.session_id === selectedSessionId);
              const { visibleSessions, hasMore } = getVisibleSessions(group);
              
              return (
                <div key={group.directory} className="session-group">
                  <div 
                    className={`group-header ${isCollapsed ? 'collapsed' : ''} ${hasSelectedSession && isCollapsed ? 'has-selected' : ''}`}
                    onClick={(e) => handleDirectoryHeaderClick(e, group.directory)}
                  >
                    <div className="group-directory" title="Click path to copy, click elsewhere to toggle">
                      <span className="collapse-icon">{isCollapsed ? '▶' : '▼'}</span>
                      <span className="directory-path" title="Click to copy path">
                        📁&nbsp;{group.directory}&nbsp;({group.sessions.length})
                      </span>
                    </div>
                  </div>
                  {!isCollapsed && (
                    <div className="group-sessions">
                      {visibleSessions.map((session) => (
                        <div
                          key={session.session_id}
                          className={`session-item ${selectedSessionId === session.session_id ? 'selected' : ''}`}
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            onSessionSelect(session.session_id);
                          }}
                        >
                          <div className="session-info">
                            <div className="session-summary" title={session.summary || `Session ${session.session_id.slice(0, 8)}...`}>
                              {truncateSummary(session.summary || `Session ${session.session_id.slice(0, 8)}...`)}
                            </div>
                            <div className="session-meta">
                              <span className="session-date">
                                {formatDate(session.latest_message_date || session.earliest_message_date)}
                              </span>
                              {session.active && <span className="session-active-indicator">● Active</span>}
                            </div>
                          </div>
                        </div>
                      ))}
                      {hasMore && (
                        <div className="show-more-container">
                          <button 
                            className="show-more-button"
                            onClick={() => showMore(group.directory)}
                          >
                            Show {Math.min(SESSIONS_PER_PAGE, group.sessions.length - visibleSessions.length)} more...
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* Directory Path Popup - only render if not controlled externally */}
      {directoryPopup && !onDirectoryPathClick && (
        <div className="directory-popup-overlay" onClick={closePopup}>
          <div className="directory-popup" onClick={(e) => e.stopPropagation()}>
            <div className="directory-popup-header">
              <h3>Directory Path</h3>
              <button className="popup-close-button" onClick={closePopup}>
                ✕
              </button>
            </div>
            <div className="directory-popup-content">
              <div className="full-path-container">
                <code className="full-path">{directoryPopup}</code>
              </div>
              <div className="popup-actions">
                <button 
                  className="copy-button"
                  onClick={() => copyToClipboard(directoryPopup)}
                >
                  {copySuccess === directoryPopup ? '✓ Copied!' : '📋 Copy Path'}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
