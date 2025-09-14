import { useState, useEffect } from 'react';
import { Routes, Route, useNavigate, useParams } from 'react-router-dom';
import { v4 as uuidv4 } from 'uuid';
import { SessionList } from './components/SessionList';
import { ChatWindow } from './components/ChatWindow';
import { HomePage } from './components/HomePage';
import { NewChatDialog } from './components/NewChatDialog';
import { useCreateSession, useSessions } from './hooks/useApi';
import type { CreateSessionRequest } from './types/api';
import './App.css';

const SIDEBAR_COLLAPSED_KEY = 'chef-de-vibe-sidebar-collapsed';

function SessionView() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const { createSession, loading: createLoading } = useCreateSession();
  const { sessions } = useSessions();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(true); // Hidden by default
  const [showNewChatDialog, setShowNewChatDialog] = useState(false);
  const [directoryPopup, setDirectoryPopup] = useState<string | null>(null);
  const [copySuccess, setCopySuccess] = useState<string | null>(null);

  // Load sidebar collapsed state from localStorage on mount
  useEffect(() => {
    const stored = localStorage.getItem(SIDEBAR_COLLAPSED_KEY);
    if (stored !== null) {
      setSidebarCollapsed(JSON.parse(stored));
    }
  }, []);


  // Save sidebar state to localStorage whenever it changes
  useEffect(() => {
    localStorage.setItem(SIDEBAR_COLLAPSED_KEY, JSON.stringify(sidebarCollapsed));
  }, [sidebarCollapsed]);

  // Handle escape key to close popup or sidebar
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        // If directory popup is open, close it first
        if (directoryPopup) {
          setDirectoryPopup(null);
          setCopySuccess(null);
        } 
        // If new chat dialog is open, let it handle its own escape
        else if (showNewChatDialog) {
          // Do nothing, let NewChatDialog handle it
        }
        // Otherwise, close the sidebar if it's open
        else if (!sidebarCollapsed) {
          setSidebarCollapsed(true);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [sidebarCollapsed, directoryPopup, showNewChatDialog]);

  // Handle swipe gestures for mobile
  useEffect(() => {
    let touchStartX = 0;
    let touchStartY = 0;
    let touchEndX = 0;
    let touchEndY = 0;
    let isSwiping = false;

    const handleTouchStart = (e: TouchEvent) => {
      if (!e.touches || e.touches.length === 0) return;
      touchStartX = e.touches[0].clientX;
      touchStartY = e.touches[0].clientY;
      isSwiping = true;
    };

    const handleTouchMove = (e: TouchEvent) => {
      if (!isSwiping || !e.touches || e.touches.length === 0) return;
      
      touchEndX = e.touches[0].clientX;
      touchEndY = e.touches[0].clientY;
    };

    const handleTouchEnd = () => {
      if (!isSwiping) return;
      isSwiping = false;

      const deltaX = touchEndX - touchStartX;
      const deltaY = touchEndY - touchStartY;
      const minSwipeDistance = 100;
      const maxVerticalMovement = 100;

      // Ignore if vertical movement is too large (likely scrolling)
      if (Math.abs(deltaY) > maxVerticalMovement) {
        return;
      }

      // Swipe right to open sidebar (when closed)
      if (deltaX > minSwipeDistance && sidebarCollapsed) {
        // Only trigger if swipe starts from left edge of screen
        if (touchStartX < 50) {
          setSidebarCollapsed(false);
        }
      }
      // Swipe left to close sidebar (when open)
      else if (deltaX < -minSwipeDistance && !sidebarCollapsed) {
        setSidebarCollapsed(true);
      }
    };

    // Add touch event listeners
    document.addEventListener('touchstart', handleTouchStart, { passive: true });
    document.addEventListener('touchmove', handleTouchMove, { passive: true });
    document.addEventListener('touchend', handleTouchEnd, { passive: true });

    return () => {
      document.removeEventListener('touchstart', handleTouchStart);
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('touchend', handleTouchEnd);
    };
  }, [sidebarCollapsed]);

  const handleSessionSelect = (sessionId: string) => {
    // Close sidebar on mobile when a chat is selected
    setSidebarCollapsed(true);
    navigate(`/session/${sessionId}`);
  };

  const handleNewChat = () => {
    // Show new chat dialog
    setShowNewChatDialog(true);
    // Don't close sidebar when opening new chat dialog
  };

  const handleStartChat = async (directory: string, firstMessage: string) => {
    setShowNewChatDialog(false);
    
    // Create the session immediately with the first message
    const newSessionId = uuidv4();
    const request: CreateSessionRequest = {
      session_id: newSessionId,
      working_dir: directory,
      resume: false,
      first_message: firstMessage
    };

    const response = await createSession(request);
    if (response) {
      // Close sidebar to give user space
      setSidebarCollapsed(true);
      // Navigate to the new session
      navigate(`/session/${response.session_id}`);
    } else {
      // Show error alert with suggestion to check backend logs
      alert('Failed to create new chat session. Please check the backend logs for more details. You may need to restart the backend service or check your working directory permissions.');
    }
  };

  const handleNewChatCancel = () => {
    setShowNewChatDialog(false);
  };

  const toggleSidebar = () => {
    setSidebarCollapsed(!sidebarCollapsed);
  };

  const handleDirectoryPathClick = (directory: string | null) => {
    setDirectoryPopup(directory);
    if (directory === null) {
      setCopySuccess(null);
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
    setDirectoryPopup(null);
    setCopySuccess(null);
  };

  const selectedSession = sessions.find(s => s.session_id === sessionId);

  return (
    <div className={`app ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
      <div className={`app-sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
        <SessionList
          selectedSessionId={sessionId || null}
          onSessionSelect={handleSessionSelect}
          onNewChat={handleNewChat}
          directoryPopup={directoryPopup}
          onDirectoryPathClick={handleDirectoryPathClick}
        />
      </div>
      
      <button 
        className={`sidebar-toggle ${sidebarCollapsed ? 'collapsed' : ''}`}
        onClick={toggleSidebar}
        title={sidebarCollapsed ? 'Show Sessions' : 'Hide Sessions'}
      ></button>
      
      <div className="app-main">
        {sessionId ? (
          <ChatWindow
            sessionId={sessionId}
            workingDirectory={selectedSession?.working_directory}
            onCreateSession={createSession}
            createLoading={createLoading}
            navigate={navigate}
            sidebarCollapsed={sidebarCollapsed}
            onNewChat={handleNewChat}
          />
        ) : (
          <HomePage
            onNewChat={handleNewChat}
            sidebarCollapsed={sidebarCollapsed}
          />
        )}      </div>

      {showNewChatDialog && (
        <NewChatDialog
          onStartChat={handleStartChat}
          onCancel={handleNewChatCancel}
        />
      )}

      {/* Directory Path Popup - rendered outside sidebar */}
      {directoryPopup && (
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
    </div>
  );
}

function App() {
  return (
    <Routes>
      <Route path="/" element={<SessionView />} />
      <Route path="/session/:sessionId" element={<SessionView />} />
    </Routes>
  );
}

export default App;