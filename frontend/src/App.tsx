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
      
      // Sidebar is open - lock scrolling on body only
      document.body.style.overflow = 'hidden';
      document.body.style.position = 'fixed';
      document.body.style.width = '100%';
      document.body.style.top = `-${scrollY}px`;
      // Don't set touchAction - let touch events decide
      
      // Also lock html element
      document.documentElement.style.overflow = 'hidden';
    } else {
      // Get the saved scroll position
      const scrollY = document.body.style.top;
      
      // Sidebar is closed - restore scrolling
      document.body.style.overflow = '';
      document.body.style.position = '';
      document.body.style.width = '';
      document.body.style.top = '';
      
      // Restore html element
      document.documentElement.style.overflow = '';
      
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
      document.documentElement.style.overflow = '';
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
    let gestureDirection: 'horizontal' | 'vertical' | null = null;
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
        // Allow swiping from anywhere on the main content OR from edge
        if (touchStartX < 20 || mainElement?.contains(target)) {
          isSwipingFromEdge = true;
          isSwiping = true;
          gestureDirection = null; // Will be determined by first move
          // DON'T lock scrolling yet - wait to see if it's horizontal
          
          if (mainElement) {
            mainElement.style.transition = 'none';
            mainElement.style.willChange = 'transform';
          }
        }
      } else {
        // Sidebar is open - allow swiping to close from overlay or sidebar
        const isOnOverlay = target.closest('.sidebar-overlay');
        const isOnSidebar = sidebarElement?.contains(target);
        
        // Only enable swipe if touching the sidebar or overlay
        if (isOnSidebar || isOnOverlay) {
          isSwipingOnChat = true;
          isSwiping = true;
          gestureDirection = null; // Will be determined by first move
          // DON'T lock scrolling yet - wait to see if it's horizontal
          
          if (mainElement) {
            mainElement.style.willChange = 'transform';
            mainElement.style.transition = 'none';
          }
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
      
      // Determine gesture direction on first significant move (if not yet determined)
      if (gestureDirection === null && (Math.abs(deltaX) > 5 || Math.abs(deltaY) > 5)) {
        // Determine if horizontal or vertical based on initial movement
        if (Math.abs(deltaX) > Math.abs(deltaY)) {
          gestureDirection = 'horizontal';
        } else {
          gestureDirection = 'vertical';
        }
      }
      
      // If gesture is determined to be vertical, cancel swipe and allow scrolling
      if (gestureDirection === 'vertical') {
        // Cancel the swipe
        isSwiping = false;
        isSwipingFromEdge = false;
        isSwipingOnChat = false;
        
        // Reset transform
        if (mainElement) {
          mainElement.style.transform = '';
          mainElement.style.transition = '';
          mainElement.style.willChange = '';
        }
        return;
      }
      
      // If gesture is horizontal, handle the swipe
      if (gestureDirection === 'horizontal' && (isSwipingFromEdge || isSwipingOnChat)) {
        e.preventDefault();
        e.stopPropagation();
        
        // Lock scrolling only when we're sure it's a horizontal swipe
        if (document.body.style.overflow !== 'hidden') {
          const scrollY = window.scrollY;
          document.body.style.overflow = 'hidden';
          document.body.style.position = 'fixed';
          document.body.style.width = '100%';
          document.body.style.top = `-${scrollY}px`;
        }
        
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
      
      // Always restore scrolling after touch ends (unless sidebar will be open)
      if (!shouldOpen && document.body.style.overflow === 'hidden') {
        // Get the saved scroll position
        const scrollY = document.body.style.top;
        
        // Sidebar will be closed - restore scrolling
        document.body.style.overflow = '';
        document.body.style.position = '';
        document.body.style.width = '';
        document.body.style.top = '';
        document.documentElement.style.overflow = '';
        
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
      gestureDirection = null;
      
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
        isSwipingFromEdge = false;
        isSwipingOnChat = false;
        gestureDirection = null;
        
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
    // If clicking on the same session, close sidebar and ensure chat is visible
    if (sessionId === newSessionId) {
      if (!sidebarCollapsed) {
        // Close sidebar with instant transition for smooth UX
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
      return; // Don't navigate, we're already on this session
    }
    
    // For different session, close sidebar immediately
    if (!sidebarCollapsed) {
      // Cancel any pending touch state updates
      interface ExtendedWindow extends Window {
        __touchTimeouts?: number[];
      }
      const extWindow = window as ExtendedWindow;
      const pendingTimeouts = extWindow.__touchTimeouts;
      if (pendingTimeouts) {
        pendingTimeouts.forEach((timeout: number) => clearTimeout(timeout));
        extWindow.__touchTimeouts = [];
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
      // Only close dialog and navigate on success
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
      
      // Navigate to the new session
      navigate(`/session/${response.session_id}`);
    } else {
      // Throw error to be caught by NewChatDialog
      throw new Error('Failed to create new chat session. Please check the backend logs for more details. You may need to restart the backend service or check your working directory permissions.');
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