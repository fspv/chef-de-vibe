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

  // Handle swipe gestures for mobile
  useEffect(() => {
    let touchStartX = 0;
    let touchStartY = 0;
    let touchEndX = 0;
    let touchEndY = 0;
    let isSwiping = false;
    let isSwipingFromEdge = false;
    let isSwipingOnSidebar = false;
    let sidebarElement: HTMLElement | null = null;
    let animationFrameId: number | null = null;
    
    // Cache DOM elements on mount
    setTimeout(() => {
      sidebarElement = document.querySelector('.app-sidebar');
    }, 0);

    const handleTouchStart = (e: TouchEvent) => {
      if (!e.touches || e.touches.length === 0) return;
      
      // Don't handle touch if it's on a clickable session item
      const target = e.target as HTMLElement;
      if (target.closest('.session-item')) {
        return; // Let the click handler handle this
      }
      
      touchStartX = e.touches[0].clientX;
      touchStartY = e.touches[0].clientY;
      touchEndX = touchStartX;
      touchEndY = touchStartY;
      
      // Quick edge check before setting up swipe
      if (touchStartX < 20 && sidebarCollapsed) {
        isSwipingFromEdge = true;
        isSwiping = true;
        // Save scroll position and lock body scrolling during swipe
        const scrollY = window.scrollY;
        document.body.style.overflow = 'hidden';
        document.body.style.position = 'fixed';
        document.body.style.width = '100%';
        document.body.style.top = `-${scrollY}px`;
        document.body.style.touchAction = 'none';
        document.documentElement.style.overflow = 'hidden';
        document.documentElement.style.touchAction = 'none';
        // Get sidebar if not cached
        if (!sidebarElement) {
          sidebarElement = document.querySelector('.app-sidebar');
        }
        if (sidebarElement) {
          // Pre-warm the sidebar by removing collapsed class but keeping it off-screen
          sidebarElement.classList.remove('collapsed');
          sidebarElement.style.transform = 'translate3d(-100%, 0, 0)';
          sidebarElement.style.transition = 'none';
          sidebarElement.style.willChange = 'transform';
        }
      } else if (!sidebarCollapsed) {
        // Check if swipe started on sidebar
        if (!sidebarElement) {
          sidebarElement = document.querySelector('.app-sidebar');
        }
        if (sidebarElement?.contains(target)) {
          isSwipingOnSidebar = true;
          isSwiping = true;
          // Save scroll position and lock body scrolling during swipe
          const scrollY = window.scrollY;
          document.body.style.overflow = 'hidden';
          document.body.style.position = 'fixed';
          document.body.style.width = '100%';
          document.body.style.top = `-${scrollY}px`;
          document.body.style.touchAction = 'none';
          document.documentElement.style.overflow = 'hidden';
          document.documentElement.style.touchAction = 'none';
          sidebarElement.style.willChange = 'transform';
          sidebarElement.style.transition = 'none';
        }
      }
    };

    const updateSwipePosition = (deltaX: number) => {
      if (!sidebarElement) return;
      
      // Handle edge swipe to open
      if (isSwipingFromEdge) {
        // Direct calculation without intermediate variables
        const translateX = Math.min(deltaX - window.innerWidth, 0);
        
        // Single transform update
        sidebarElement.style.transform = `translateX(${translateX}px)`;
      }
      
      // Handle swipe on sidebar to close
      if (isSwipingOnSidebar && deltaX < 0) {
        // Direct pixel-based transform
        sidebarElement.style.transform = `translateX(${deltaX}px)`;
      }
    };

    const handleTouchMove = (e: TouchEvent) => {
      if (!isSwiping || !e.touches || e.touches.length === 0) return;
      if (!sidebarElement) return;
      
      touchEndX = e.touches[0].clientX;
      touchEndY = e.touches[0].clientY;
      
      const deltaX = touchEndX - touchStartX;
      const deltaY = touchEndY - touchStartY;
      
      // Always prevent default scrolling when swiping the sidebar
      if (isSwipingFromEdge || isSwipingOnSidebar) {
        e.preventDefault();
        e.stopPropagation();
        
        // If vertical movement is too significant compared to horizontal, still prevent but don't update position
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
      } else if (isSwipingOnSidebar) {
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
      // If shouldOpen is true, keep scrolling locked (will be handled by the useEffect)
      
      // Immediately set final transform position
      if (sidebarElement) {
        // Clear will-change and set final position
        sidebarElement.style.willChange = '';
        sidebarElement.style.transform = shouldOpen ? '' : 'translate3d(-100%, 0, 0)';
        // Re-enable transition after next frame
        requestAnimationFrame(() => {
          if (sidebarElement) {
            sidebarElement.style.transition = '';
          }
        });
      }
      
      // Update overlay
      const overlay = document.querySelector('.sidebar-overlay') as HTMLElement;
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
      
      // Update React state much later
      const timeoutId = setTimeout(() => setSidebarCollapsed(!shouldOpen), 150);
      // Track timeout for potential cancellation
      if (!(window as any).__touchTimeouts) {
        (window as any).__touchTimeouts = [];
      }
      (window as any).__touchTimeouts.push(timeoutId);
      
      // Reset all flags
      isSwiping = false;
      isSwipingFromEdge = false;
      isSwipingOnSidebar = false;
      
      // Cancel any pending frames
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
      }
    };

    const handleTouchCancel = () => {
      // Clean up on cancel (e.g., when another app takes focus)
      if (isSwiping) {
        isSwiping = false;
        
        // Restore body scrolling if sidebar is collapsed
        if (sidebarCollapsed) {
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
        }
        
        // Reset sidebar
        if (sidebarElement) {
          sidebarElement.style.transition = '';
          sidebarElement.style.transform = '';
          sidebarElement.style.willChange = '';
        }
        
        // Remove temporary overlay
        const tempOverlay = document.querySelector('.sidebar-overlay-temp');
        if (tempOverlay) {
          tempOverlay.remove();
        }
        
        // Reset regular overlay
        const regularOverlay = document.querySelector('.sidebar-overlay') as HTMLElement;
        if (regularOverlay) {
          regularOverlay.style.transition = '';
          regularOverlay.style.opacity = '';
          regularOverlay.style.pointerEvents = '';
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
      
      // Clean up any temporary overlay
      const tempOverlay = document.querySelector('.sidebar-overlay-temp');
      if (tempOverlay) {
        tempOverlay.remove();
      }
    };
  }, [sidebarCollapsed]);

  const handleSessionSelect = (sessionId: string) => {
    // Set sidebar state FIRST to avoid race conditions
    // This ensures the state is updated before navigation causes re-renders
    if (!sidebarCollapsed) {
      // Cancel any pending touch state updates
      const pendingTimeouts = (window as any).__touchTimeouts;
      if (pendingTimeouts) {
        pendingTimeouts.forEach((timeout: number) => clearTimeout(timeout));
        (window as any).__touchTimeouts = [];
      }
      
      setSidebarCollapsed(true);
    }
    
    // Navigate after state is set
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
      bootstrap_messages: [firstMessage]
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


  return (
    <div className={`app ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
      {/* Sidebar overlay for mobile - always in DOM, visibility controlled by CSS/JS */}
      <div 
        className="sidebar-overlay"
        style={{
          display: sidebarCollapsed ? 'none' : 'block',
          opacity: sidebarCollapsed ? 0 : 1,
          pointerEvents: sidebarCollapsed ? 'none' : 'auto'
        }}
        onClick={() => setSidebarCollapsed(true)}
      />
      
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