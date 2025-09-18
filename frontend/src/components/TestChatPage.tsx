import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { MessageList, type MessageListRef } from './MessageList';
import { MessageInput } from './MessageInput';
import { SessionList } from './SessionList';
import { SessionStatusIndicator } from './SessionStatusIndicator';

const testMessages = [
  // System initialization message
  {
    id: "sys-msg-1",
    type: "system",
    subtype: "init",
    model: "claude-3-5-sonnet-20241022",
    permissionMode: "ask",
    apiKeySource: "env",
    cwd: "/home/user/test-project",
    tools: ["Bash", "Edit", "Write", "Read", "Task", "Glob", "Grep", "TodoWrite"],
    mcp_servers: [
      { name: "filesystem", status: "connected" },
      { name: "database", status: "connected" }
    ],
    timestamp: Date.now() - 10000
  },

  // User message
  {
    id: "user-msg-1",
    type: "user",
    message: {
      content: [
        {
          type: "text",
          text: "Please help me implement a new feature that tracks user analytics and exports data to CSV. Show me examples of all the different tools you can use."
        }
      ]
    },
    timestamp: Date.now() - 9000
  },

  // Assistant message with various tool uses
  {
    id: "asst-msg-1", 
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "I'll help you implement a user analytics feature with data export capabilities. Let me start by creating a todo list to track our progress, then I'll demonstrate various tools I can use."
        },
        {
          type: "tool_use",
          id: "todo-123",
          name: "TodoWrite",
          input: {
            todos: [
              {
                content: "Create analytics database schema",
                status: "pending",
                priority: "high",
                id: "1"
              },
              {
                content: "Implement data collection service",
                status: "in_progress", 
                priority: "high",
                id: "2"
              },
              {
                content: "Build CSV export functionality",
                status: "completed",
                priority: "medium",
                id: "3"
              }
            ]
          }
        }
      ]
    },
    timestamp: Date.now() - 8000
  },

  // Assistant message with Agent/Task tool
  {
    id: "asst-msg-2",
    type: "assistant", 
    message: {
      content: [
        {
          type: "text",
          text: "Let me use an agent to research best practices for analytics implementation:"
        },
        {
          type: "tool_use",
          id: "agent-123",
          name: "Task",
          input: {
            description: "Research analytics patterns",
            prompt: "Research and analyze modern user analytics implementation patterns, focusing on privacy-compliant data collection and efficient export mechanisms. Provide recommendations for database schema design and data processing pipelines.",
            subagent_type: "research-specialist"
          }
        }
      ]
    },
    timestamp: Date.now() - 7000
  },

  // Assistant message with file operations
  {
    id: "asst-msg-3",
    type: "assistant",
    message: {
      content: [
        {
          type: "text", 
          text: "Now I'll create the initial analytics service file:"
        },
        {
          type: "tool_use",
          id: "write-123",
          name: "Write",
          input: {
            file_path: "/home/user/test-project/src/analytics/service.py",
            content: "from datetime import datetime\nfrom typing import Dict, List, Optional\nimport csv\nimport json\n\nclass AnalyticsService:\n    def __init__(self, db_connection):\n        self.db = db_connection\n        self.events = []\n    \n    def track_event(self, user_id: str, event_name: str, properties: Dict = None):\n        \"\"\"Track a user event with optional properties\"\"\"\n        event = {\n            'user_id': user_id,\n            'event_name': event_name,\n            'properties': properties or {},\n            'timestamp': datetime.utcnow().isoformat()\n        }\n        self.events.append(event)\n        return self._save_event(event)\n    \n    def export_to_csv(self, filename: str, date_range: Optional[tuple] = None):\n        \"\"\"Export analytics data to CSV file\"\"\"\n        events = self._get_events(date_range)\n        \n        with open(filename, 'w', newline='') as csvfile:\n            fieldnames = ['user_id', 'event_name', 'timestamp', 'properties']\n            writer = csv.DictWriter(csvfile, fieldnames=fieldnames)\n            \n            writer.writeheader()\n            for event in events:\n                event['properties'] = json.dumps(event['properties'])\n                writer.writerow(event)\n    \n    def _save_event(self, event: Dict) -> bool:\n        # Implementation for saving to database\n        pass\n    \n    def _get_events(self, date_range: Optional[tuple] = None) -> List[Dict]:\n        # Implementation for retrieving events\n        pass"
          }
        }
      ]
    },
    timestamp: Date.now() - 6000
  },

  // Assistant message with single edit operation
  {
    id: "asst-msg-4",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me fix a small issue in the analytics service - I need to add proper error handling:"
        },
        {
          type: "tool_use",
          id: "edit-123",
          name: "Edit",
          input: {
            file_path: "/home/user/test-project/src/analytics/service.py",
            old_string: "    def _save_event(self, event: Dict) -> bool:\n        # Implementation for saving to database\n        pass",
            new_string: "    def _save_event(self, event: Dict) -> bool:\n        \"\"\"Save event to database with proper error handling\"\"\"\n        try:\n            # Implementation for saving to database\n            return True\n        except Exception as e:\n            print(f\"Failed to save event: {e}\")\n            return False",
            replace_all: false
          }
        }
      ]
    },
    timestamp: Date.now() - 5500
  },

  // Assistant message with search operations
  {
    id: "asst-msg-5",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Now let me search for existing analytics code in the project:"
        },
        {
          type: "tool_use",
          id: "glob-123", 
          name: "Glob",
          input: {
            pattern: "**/*analytics*.py",
            path: "/home/user/test-project"
          }
        }
      ]
    },
    timestamp: Date.now() - 5000
  },

  // Assistant message with Grep search
  {
    id: "asst-msg-6",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Now let me search for any existing tracking code:"
        },
        {
          type: "tool_use",
          id: "grep-123",
          name: "Grep", 
          input: {
            pattern: "track_event|analytics|metrics",
            path: "/home/user/test-project/src",
            glob: "*.py",
            output_mode: "content",
            "-i": true,
            multiline: false,
            type: "py"
          }
        }
      ]
    },
    timestamp: Date.now() - 4000
  },

  // Assistant message with bash command
  {
    id: "asst-msg-7",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me install the required dependencies:"
        },
        {
          type: "tool_use",
          id: "bash-123",
          name: "Bash",
          input: {
            command: "pip install pandas matplotlib seaborn sqlalchemy",
            description: "Install analytics and data processing libraries",
            timeout: 30000,
            run_in_background: false
          }
        }
      ]
    },
    timestamp: Date.now() - 3000
  },

  // Assistant message with multi-edit
  {
    id: "asst-msg-8",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Now I'll make several improvements to the analytics service:"
        },
        {
          type: "tool_use",
          id: "multi-edit-123",
          name: "MultiEdit",
          input: {
            file_path: "/home/user/test-project/src/analytics/service.py",
            edits: [
              {
                old_string: "import csv\nimport json",
                new_string: "import csv\nimport json\nimport pandas as pd\nfrom sqlalchemy import create_engine",
                replace_all: false
              },
              {
                old_string: "def __init__(self, db_connection):\n        self.db = db_connection\n        self.events = []",
                new_string: "def __init__(self, db_connection):\n        self.db = db_connection\n        self.events = []\n        self.engine = create_engine(db_connection)",
                replace_all: false
              },
              {
                old_string: "# Implementation for saving to database\n        pass",
                new_string: "try:\n            # Save event to database\n            query = \"INSERT INTO events (user_id, event_name, properties, timestamp) VALUES (?, ?, ?, ?)\"\n            self.db.execute(query, (event['user_id'], event['event_name'], json.dumps(event['properties']), event['timestamp']))\n            return True\n        except Exception as e:\n            print(f\"Error saving event: {e}\")\n            return False",
                replace_all: false
              }
            ]
          }
        }
      ]
    },
    timestamp: Date.now() - 2000
  },

  // Assistant message with notebook editing
  {
    id: "asst-msg-9",
    type: "assistant", 
    message: {
      content: [
        {
          type: "text",
          text: "Let me create a data analysis notebook:"
        },
        {
          type: "tool_use",
          id: "notebook-123",
          name: "NotebookEdit",
          input: {
            notebook_path: "/home/user/test-project/analytics_dashboard.ipynb",
            cell_id: "cell-456",
            new_source: "import pandas as pd\nimport matplotlib.pyplot as plt\nimport seaborn as sns\nfrom analytics.service import AnalyticsService\n\n# Load and analyze user analytics data\nanalytics = AnalyticsService('postgresql://localhost/analytics')\ndf = pd.read_sql('SELECT * FROM events WHERE timestamp >= NOW() - INTERVAL 30 days', analytics.engine)\n\n# Create visualizations\nplt.figure(figsize=(12, 6))\nsns.countplot(data=df, x='event_name', order=df['event_name'].value_counts().index)\nplt.xticks(rotation=45)\nplt.title('Event Frequency - Last 30 Days')\nplt.tight_layout()\nplt.show()",
            cell_type: "code",
            edit_mode: "replace"
          }
        }
      ]
    },
    timestamp: Date.now() - 1000
  },

  // Web operations messages
  {
    id: "asst-msg-10",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me research the latest analytics best practices:"
        },
        {
          type: "tool_use",
          id: "search-123", 
          name: "WebSearch",
          input: {
            query: "user analytics privacy GDPR best practices 2024",
            allowed_domains: ["gdpr.eu", "analytics.google.com", "segment.com"],
            blocked_domains: ["spam-site.com"]
          }
        }
      ]
    },
    timestamp: Date.now() - 500
  },

  // Web fetch message
  {
    id: "asst-msg-11",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me fetch documentation for the pandas library:"
        },
        {
          type: "tool_use",
          id: "fetch-123",
          name: "WebFetch", 
          input: {
            url: "https://pandas.pydata.org/docs/reference/api/pandas.DataFrame.to_csv.html",
            prompt: "Extract information about CSV export options and parameters, focusing on performance optimization for large datasets"
          }
        }
      ]
    },
    timestamp: Date.now() - 400
  },

  // File read operation
  {
    id: "asst-msg-12", 
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me check the configuration file:"
        },
        {
          type: "tool_use",
          id: "read-123",
          name: "Read",
          input: {
            file_path: "/home/user/test-project/config/analytics.json",
            offset: 10,
            limit: 50
          }
        }
      ]
    },
    timestamp: Date.now() - 300
  },

  // MCP operations
  {
    id: "asst-msg-13",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me list available MCP resources:"
        },
        {
          type: "tool_use",
          id: "mcp-list-123",
          name: "ListMcpResources",
          input: {
            server: "database-server"
          }
        }
      ]
    },
    timestamp: Date.now() - 200
  },

  // Read MCP resource
  {
    id: "asst-msg-14",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Now let me read user data from the database:"
        },
        {
          type: "tool_use",
          id: "mcp-read-123", 
          name: "ReadMcpResource",
          input: {
            server: "database-server",
            uri: "postgresql://localhost/analytics/users"
          }
        }
      ]
    },
    timestamp: Date.now() - 100
  },

  // Generic MCP operation
  {
    id: "asst-msg-15",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me perform a custom database operation:"
        },
        {
          type: "tool_use",
          id: "mcp-123",
          name: "Mcp",
          input: {
            operation: "query",
            target: "analytics_db",
            parameters: {
              table: "events",
              filter: "timestamp >= '2024-01-01'",
              aggregate: "COUNT(*) as total_events",
              group_by: "event_name"
            }
          }
        }
      ]
    },
    timestamp: Date.now() - 50
  },

  // Background shell operations
  {
    id: "asst-msg-16",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Let me start a background data processing job:"
        },
        {
          type: "tool_use",
          id: "bash-bg-123",
          name: "Bash",
          input: {
            command: "python scripts/process_analytics_data.py --date=2024-01-01 --output=/tmp/analytics_report.csv",
            description: "Process analytics data in background",
            timeout: 300000,
            run_in_background: true
          }
        }
      ]
    },
    timestamp: Date.now() - 25
  },

  // BashOutput tool
  {
    id: "asst-msg-17",
    type: "assistant",
    message: {
      content: [
        {
          type: "text", 
          text: "Let me check the output from the background process:"
        },
        {
          type: "tool_use",
          id: "bash-output-123",
          name: "BashOutput",
          input: {
            bash_id: "bg-shell-456",
            filter: "progress|complete|error"
          }
        }
      ]
    },
    timestamp: Date.now() - 10
  },

  // Plan mode and cleanup
  {
    id: "asst-msg-18",
    type: "assistant",
    message: {
      content: [
        {
          type: "text",
          text: "Perfect! I've demonstrated all the tools available. Here's a summary of what we accomplished:"
        },
        {
          type: "tool_use",
          id: "plan-123",
          name: "ExitPlanMode",
          input: {
            plan: "## Analytics Implementation Summary\n\n### ✅ Completed Tasks\n\n1. **Project Setup**\n   - Created analytics service with database integration\n   - Installed required dependencies (pandas, matplotlib, seaborn, sqlalchemy)\n   - Set up proper file structure\n\n2. **Core Features Implemented**\n   - Event tracking functionality\n   - CSV export capabilities with date filtering\n   - Database persistence layer\n   - Error handling and logging\n\n3. **Data Analysis Tools**\n   - Created Jupyter notebook for data visualization\n   - Implemented event frequency analysis\n   - Added charts and graphs for insights\n\n4. **Integration & Testing**\n   - Connected to MCP database resources\n   - Set up background data processing\n   - Configured analytics dashboard\n\n### 🛠️ Tools Demonstrated\n\n- **File Operations**: Write, Edit, MultiEdit, Read\n- **Search & Discovery**: Glob, Grep, WebSearch, WebFetch  \n- **System Operations**: Bash (foreground & background), BashOutput, KillShell\n- **Task Management**: TodoWrite, Task (Agent), ExitPlanMode\n- **Data Operations**: NotebookEdit, ListMcpResources, ReadMcpResource, Mcp\n\n### 🚀 Next Steps\n\n- Test the analytics service with real user data\n- Set up automated reports and dashboards\n- Implement privacy compliance features\n- Add real-time analytics capabilities"
          }
        }
      ]
    },
    timestamp: Date.now()
  },

  // Success result message
  {
    id: "result-msg-1",
    type: "result", 
    subtype: "success",
    result: "Analytics implementation completed successfully! The system is now ready to track user events and export data to CSV format. All tools have been demonstrated with practical examples.",
    duration_ms: 2500,
    duration_api_ms: 1200,
    num_turns: 15,
    total_cost_usd: 0.0245,
    timestamp: Date.now() + 100
  }
];

const SIDEBAR_COLLAPSED_KEY = 'chef-de-vibe-sidebar-collapsed';

export function TestChatPage() {
  const navigate = useNavigate();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(true);
  const [debugMode, setDebugMode] = useState(false);
  const [autoScrollPaused, setAutoScrollPaused] = useState(false);
  const messageListRef = useRef<MessageListRef>(null);
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
        if (directoryPopup) {
          setDirectoryPopup(null);
          setCopySuccess(null);
        } else if (!sidebarCollapsed) {
          setSidebarCollapsed(true);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [sidebarCollapsed, directoryPopup]);

  const handleSessionSelect = (sessionId: string) => {
    setSidebarCollapsed(true);
    navigate(`/session/${sessionId}`);
  };

  const handleNewChat = () => {
    navigate('/');
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

  const handleSendMessage = (message: string) => {
    // Just log the message for demo purposes
    console.log('Test message sent:', message);
    alert(`Demo: Would send message "${message}" - but this is just a test page`);
  };

  const handleApprove = (requestId: string, input: unknown, permissionUpdates: unknown[] = []) => {
    console.log('Test approval:', { requestId, input, permissionUpdates });
    alert(`Demo: Approved request ${requestId}`);
  };

  const handleDeny = (requestId: string) => {
    console.log('Test denial:', requestId);
    alert(`Demo: Denied request ${requestId}`);
  };


  return (
    <div className={`app ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
      <div className={`app-sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
        <SessionList
          selectedSessionId="test"
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
        <div className="chat-window">
          <div className="chat-header">
            <div className="session-info">
              <div className="session-title">
                <h2>🧪 Test Chat - All Message Types Demo</h2>
                <p className="session-subtitle">Demonstrating all Claude Code SDK tool types</p>
              </div>
            </div>
            
            <div className="header-controls">
              <SessionStatusIndicator 
                isActive={true}
                isMainConnected={true}
                isApprovalConnected={true}
                hasApprovalRequests={false}
                approvalRequestCount={0}
                sessionId="test"
                workingDirectory="/home/user/test-project"
                debugMode={debugMode}
                onDebugModeChange={setDebugMode}
                autoScrollPaused={autoScrollPaused}
                onToggleAutoScroll={() => setAutoScrollPaused(!autoScrollPaused)}
              />
              
              <button 
                className="new-chat-button"
                onClick={handleNewChat}
                title="Start New Chat"
              >
                ✨ New Chat
              </button>
            </div>
          </div>

          <div className="chat-content">
            <MessageList
              ref={messageListRef}
              sessionMessages={[]}
              webSocketMessages={testMessages.map(msg => ({ 
                data: msg, 
                timestamp: msg.timestamp || Date.now() 
              }))}
              debugMode={debugMode}
              onApprove={handleApprove}
              onDeny={handleDeny}
            />
          </div>

          <div className="chat-input-container">
            <MessageInput
              onSendMessage={handleSendMessage}
              disabled={false}
              debugMode={debugMode}
            />
          </div>
        </div>
      </div>

      {/* Directory Path Popup - same as regular chat */}
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