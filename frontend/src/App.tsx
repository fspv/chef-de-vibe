import { useState, useEffect } from 'react';
import { Routes, Route, useNavigate, useParams } from 'react-router-dom';
import { v4 as uuidv4 } from 'uuid';
import { SessionList } from './components/SessionList';
import { ChatWindow } from './components/ChatWindow';
import { HomePage } from './components/HomePage';
import { NewChatDialog } from './components/NewChatDialog';
import { TestChatPage } from './components/TestChatPage';
import { useCreateSession } from './hooks/useApi';
import type { CreateSessionRequest } from './types/api';
import './App.css';

const SIDEBAR_COLLAPSED_KEY = 'chef-de-vibe-sidebar-collapsed';

function SessionView() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const { createSession, loading: createLoading } = useCreateSession();
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

  // Lock body scrolling when sidebar is open
  useEffect(() => {
    if (!sidebarCollapsed) {
      // Save current scroll position
      const scrollY = window.scrollY;
      
      // Sidebar is open - lock scrolling
      document.body.style.overflow = 'hidden';
      document.body.style.position = 'fixed';
      document.body.style.width = '100%';
      document.body.style.top = `-${scrollY}px`;
      document.body.style.touchAction = 'none';
      
      // Also lock html element
      document.documentElement.style.overflow = 'hidden';
      document.documentElement.style.touchAction = 'none';
    } else {
      // Get the saved scroll position
      const scrollY = document.body.style.top;
      
      // Sidebar is closed - restore scrolling
      document.body.style.overflow = '';
      document.body.style.position = '';
      document.body.style.width = '';
      document.body.style.top = '';
      document.body.style.touchAction = '';
      
      // Restore html element
      document.documentElement.style.overflow = '';
      document.documentElement.style.touchAction = '';
      
      // Restore scroll position
      if (scrollY) {
        window.scrollTo(0, parseInt(scrollY.replace('-', '').replace('px', ''), 10));
      }
    }

    // Cleanup on unmount
    return () => {
      document.body.style.overflow = '';
      document.body.style.position = '';
      document.body.style.width = '';
      document.body.style.top = '';
      document.body.style.touchAction = '';
      document.documentElement.style.overflow = '';
      document.documentElement.style.touchAction = '';
    };
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

  // Handle swipe gestures for mobile - Telegram-like behavior
  useEffect(() => {
    let touchStartX = 0;
    let touchStartY = 0;
    let touchEndX = 0;
    let touchEndY = 0;
    let isSwiping = false;
    let isSwipingFromEdge = false;
    let isSwipingOnChat = false;
    let mainElement: HTMLElement | null = null;
    let sidebarElement: HTMLElement | null = null;
    let animationFrameId: number | null = null;
    
    // Cache DOM elements on mount
    setTimeout(() => {
      mainElement = document.querySelector('.app-main');
      sidebarElement = document.querySelector('.app-sidebar');
    }, 0);

    const handleTouchStart = (e: TouchEvent) => {
      if (!e.touches || e.touches.length === 0) return;
      
      const target = e.target as HTMLElement;
      
      // Don't interfere with clicks on interactive elements
      if (target.closest('.session-item, .group-header, button, a, input, textarea, .show-more-button')) {
        return;
      }
      
      touchStartX = e.touches[0].clientX;
      touchStartY = e.touches[0].clientY;
      touchEndX = touchStartX;
      touchEndY = touchStartY;
      
      // Get elements if not cached
      if (!mainElement) {
        mainElement = document.querySelector('.app-main');
      }
      if (!sidebarElement) {
        sidebarElement = document.querySelector('.app-sidebar');
      }
      
      // Telegram-like behavior: drag the main content to reveal sidebar
      if (sidebarCollapsed) {
        // Check if starting drag from edge or from main content
        if (touchStartX < 20 || mainElement?.contains(target)) {
          isSwipingFromEdge = true;
          isSwiping = true;
          // Lock body scrolling during swipe
          const scrollY = window.scrollY;
          document.body.style.overflow = 'hidden';
          document.body.style.position = 'fixed';
          document.body.style.width = '100%';
          document.body.style.top = `-${scrollY}px`;
          document.body.style.touchAction = 'none';
          document.documentElement.style.overflow = 'hidden';
          document.documentElement.style.touchAction = 'none';
          
          if (mainElement) {
            mainElement.style.transition = 'none';
            mainElement.style.willChange = 'transform';
          }
        }
      } else {
        // Sidebar is open - allow swiping from anywhere (sidebar or main content)
        isSwipingOnChat = true;
        isSwiping = true;
        // Lock body scrolling during swipe
        const scrollY = window.scrollY;
        document.body.style.overflow = 'hidden';
        document.body.style.position = 'fixed';
        document.body.style.width = '100%';
        document.body.style.top = `-${scrollY}px`;
        document.body.style.touchAction = 'none';
        document.documentElement.style.overflow = 'hidden';
        document.documentElement.style.touchAction = 'none';
        
        if (mainElement) {
          mainElement.style.willChange = 'transform';
          mainElement.style.transition = 'none';
        }
      }
    };

    const updateSwipePosition = (deltaX: number) => {
      if (!mainElement) return;
      
      // Handle swipe to open sidebar (drag main content right)
      if (isSwipingFromEdge && deltaX > 0) {
        // Limit the drag to the width of the viewport
        const translateX = Math.min(deltaX, window.innerWidth);
        mainElement.style.transform = `translateX(${translateX}px)`;
        
        // Update overlay opacity based on drag progress
        const overlay = mainElement.querySelector('.sidebar-overlay') as HTMLElement;
        if (overlay) {
          const progress = translateX / window.innerWidth;
          overlay.style.display = 'block';
          overlay.style.opacity = String(progress);
        }
      }
      
      // Handle swipe to close sidebar
      if (isSwipingOnChat) {
        // Start from open position (100vw) and apply delta
        const translateX = Math.min(Math.max(window.innerWidth + deltaX, 0), window.innerWidth);
        mainElement.style.transform = `translateX(${translateX}px)`;
        
        // Update overlay opacity based on drag progress
        const overlay = mainElement.querySelector('.sidebar-overlay') as HTMLElement;
        if (overlay) {
          const progress = translateX / window.innerWidth;
          overlay.style.opacity = String(progress);
        }
      }
    };

    const handleTouchMove = (e: TouchEvent) => {
      if (!isSwiping || !e.touches || e.touches.length === 0) return;
      if (!mainElement) return;
      
      touchEndX = e.touches[0].clientX;
      touchEndY = e.touches[0].clientY;
      
      const deltaX = touchEndX - touchStartX;
      const deltaY = touchEndY - touchStartY;
      
      // Prevent default scrolling when swiping
      if (isSwipingFromEdge || isSwipingOnChat) {
        e.preventDefault();
        e.stopPropagation();
        
        // If vertical movement is too significant, still prevent but don't update position
        if (Math.abs(deltaY) > Math.abs(deltaX) * 2 && Math.abs(deltaY) > 50) {
          return;
        }
        
        // Only update position if there's meaningful horizontal movement
        if (Math.abs(deltaX) > 5) {
          // Skip if we already have a pending update
          if (animationFrameId !== null) {
            return;
          }
          
          // Use requestAnimationFrame for smooth updates
          animationFrameId = requestAnimationFrame(() => {
            updateSwipePosition(deltaX);
            animationFrameId = null;
          });
        }
      }
    };

    const handleTouchEnd = () => {
      if (!isSwiping) return;
      
      const deltaX = touchEndX - touchStartX;
      const threshold = window.innerWidth * 0.25;
      
      // Determine final state
      let shouldOpen = false;
      if (isSwipingFromEdge) {
        shouldOpen = deltaX > threshold;
      } else if (isSwipingOnChat) {
        shouldOpen = deltaX > -threshold;
      }
      
      // Update body scrolling based on final state
      if (!shouldOpen) {
        // Get the saved scroll position
        const scrollY = document.body.style.top;
        
        // Sidebar will be closed - restore scrolling
        document.body.style.overflow = '';
        document.body.style.position = '';
        document.body.style.width = '';
        document.body.style.top = '';
        document.body.style.touchAction = '';
        document.documentElement.style.overflow = '';
        document.documentElement.style.touchAction = '';
        
        // Restore scroll position
        if (scrollY) {
          window.scrollTo(0, parseInt(scrollY.replace('-', '').replace('px', ''), 10));
        }
      }
      
      // Set final transform position on main content
      if (mainElement) {
        // Clear will-change and set final position
        mainElement.style.willChange = '';
        mainElement.style.transform = shouldOpen ? 'translateX(100vw)' : 'translateX(0)';
        // Re-enable transition after next frame
        requestAnimationFrame(() => {
          if (mainElement) {
            mainElement.style.transition = '';
          }
        });
      }
      
      // Update overlay
      const overlay = mainElement?.querySelector('.sidebar-overlay') as HTMLElement;
      if (overlay) {
        overlay.style.display = shouldOpen ? 'block' : 'none';
        overlay.style.opacity = shouldOpen ? '1' : '0';
      }
      
      // Update classes
      const appElement = document.querySelector('.app');
      if (appElement) {
        if (shouldOpen) {
          sidebarElement?.classList.remove('collapsed');
          appElement.classList.remove('sidebar-collapsed');
        } else {
          sidebarElement?.classList.add('collapsed');
          appElement.classList.add('sidebar-collapsed');
        }
      }
      
      // Update React state
      setTimeout(() => setSidebarCollapsed(!shouldOpen), 150);
      
      // Reset all flags
      isSwiping = false;
      isSwipingFromEdge = false;
      isSwipingOnChat = false;
      
      // Cancel any pending frames
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
      }
    };

    const handleTouchCancel = () => {
      // Clean up on cancel
      if (isSwiping) {
        isSwiping = false;
        
        // Restore body scrolling
        const scrollY = document.body.style.top;
        document.body.style.overflow = '';
        document.body.style.position = '';
        document.body.style.width = '';
        document.body.style.top = '';
        document.body.style.touchAction = '';
        document.documentElement.style.overflow = '';
        document.documentElement.style.touchAction = '';
        
        // Restore scroll position
        if (scrollY) {
          window.scrollTo(0, parseInt(scrollY.replace('-', '').replace('px', ''), 10));
        }
        
        // Reset main element
        if (mainElement) {
          mainElement.style.transition = '';
          mainElement.style.transform = '';
          mainElement.style.willChange = '';
        }
        
        // Reset overlay
        const overlay = mainElement?.querySelector('.sidebar-overlay') as HTMLElement;
        if (overlay) {
          overlay.style.transition = '';
          overlay.style.opacity = '';
          overlay.style.display = '';
          overlay.style.pointerEvents = '';
        }
        
        // Cancel any pending animation frame
        if (animationFrameId !== null) {
          cancelAnimationFrame(animationFrameId);
          animationFrameId = null;
        }
      }
    };

    // Add touch event listeners with non-passive for touchmove to allow preventDefault
    document.addEventListener('touchstart', handleTouchStart, { passive: true });
    document.addEventListener('touchmove', handleTouchMove, { passive: false });
    document.addEventListener('touchend', handleTouchEnd, { passive: true });
    document.addEventListener('touchcancel', handleTouchCancel, { passive: true });

    return () => {
      document.removeEventListener('touchstart', handleTouchStart);
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('touchend', handleTouchEnd);
      document.removeEventListener('touchcancel', handleTouchCancel);
      
      // Clean up any pending animation frame
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
      }
      
    };
  }, [sidebarCollapsed]);

  const handleSessionSelect = (newSessionId: string) => {
    // If clicking on the same session, just close the sidebar
    if (sessionId === newSessionId) {
      if (!sidebarCollapsed) {
        setSidebarCollapsed(true);
      }
      return; // Don't navigate, we're already on this session
    }
    
    // For different session, close sidebar immediately
    if (!sidebarCollapsed) {
      // Cancel any pending touch state updates
      const pendingTimeouts = (window as any).__touchTimeouts;
      if (pendingTimeouts) {
        pendingTimeouts.forEach((timeout: number) => clearTimeout(timeout));
        (window as any).__touchTimeouts = [];
      }
      
      // Close sidebar instantly without animation for smooth transition
      const mainElement = document.querySelector('.app-main') as HTMLElement;
      const overlay = mainElement?.querySelector('.sidebar-overlay') as HTMLElement;
      
      if (mainElement) {
        mainElement.style.transition = 'none';
        mainElement.style.transform = 'translateX(0)';
      }
      
      if (overlay) {
        overlay.style.display = 'none';
      }
      
      setSidebarCollapsed(true);
      
      // Re-enable transitions after a frame
      requestAnimationFrame(() => {
        if (mainElement) {
          mainElement.style.transition = '';
        }
      });
    }
    
    // Navigate to the new session
    navigate(`/session/${newSessionId}`);
  };

  const handleNewChat = () => {
    // Show new chat dialog
    setShowNewChatDialog(true);
    // Don't close sidebar when opening new chat dialog
  };

  const handleStartChat = async (directory: string, firstMessage: string) => {
    setShowNewChatDialog(false);
    
    // Close sidebar immediately for smooth transition
    if (!sidebarCollapsed) {
      const mainElement = document.querySelector('.app-main') as HTMLElement;
      const overlay = mainElement?.querySelector('.sidebar-overlay') as HTMLElement;
      
      if (mainElement) {
        mainElement.style.transition = 'none';
        mainElement.style.transform = 'translateX(0)';
      }
      
      if (overlay) {
        overlay.style.display = 'none';
      }
      
      setSidebarCollapsed(true);
      
      // Re-enable transitions after a frame
      requestAnimationFrame(() => {
        if (mainElement) {
          mainElement.style.transition = '';
        }
      });
    }
    
    // Create the session immediately with the first message
    const newSessionId = uuidv4();
    const request: CreateSessionRequest = {
      session_id: newSessionId,
      working_dir: directory,
      resume: false,
      bootstrap_messages: [firstMessage]
    };

    const response = await createSession(request);
    if (response) {
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
        {/* Overlay attached to main content so it moves with it */}
        <div 
          className="sidebar-overlay"
          style={{
            display: sidebarCollapsed ? 'none' : 'block',
            opacity: sidebarCollapsed ? 0 : 1,
          }}
          onClick={() => setSidebarCollapsed(true)}
        />
        {sessionId ? (
          <ChatWindow
            sessionId={sessionId}
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
      <Route path="/session/test" element={<TestChatPage />} />
      <Route path="/session/:sessionId" element={<SessionView />} />
    </Routes>
  );
}

export default App;