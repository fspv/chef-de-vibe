interface HomePageProps {
  onNewChat: () => void;
  sidebarCollapsed: boolean;
}

export function HomePage({ onNewChat, sidebarCollapsed }: HomePageProps) {
  return (
    <div className="home-page">
      <div className="home-content">
        <div className="home-header">
          <h1>Chef de Vibe</h1>
          <p>Your AI coding assistant powered by Claude</p>
        </div>
        
        <div className="home-actions">
          <button 
            className="primary-button"
            onClick={onNewChat}
          >
            Start New Chat
          </button>
        </div>
        
        <div className="home-info">
          <div className="info-section">
            <h3>Get Started</h3>
            <p>Click "Start New Chat" to begin a new conversation with Claude. You'll be able to select a working directory where Claude can read and write files.</p>
          </div>
          
          <div className="info-section">
            <h3>Recent Sessions</h3>
            <p>{sidebarCollapsed ? 'Open the sidebar to view' : 'View'} your recent chat sessions and resume previous conversations.</p>
          </div>
        </div>
      </div>
    </div>
  );
}