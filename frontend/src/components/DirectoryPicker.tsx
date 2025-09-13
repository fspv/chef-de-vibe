import { useState, useEffect, useMemo, useRef } from 'react';
import { useSessions } from '../hooks/useApi';

interface DirectoryPickerProps {
  value: string;
  onChange: (directory: string) => void;
  placeholder?: string;
  className?: string;
}

interface DirectoryOption {
  path: string;
  lastUsed: string | null;
  sessionCount: number;
}

export function DirectoryPicker({ value, onChange, placeholder = "Select or type a directory...", className = "" }: DirectoryPickerProps) {
  const { sessions } = useSessions();
  const [isOpen, setIsOpen] = useState(false);
  const [filterText, setFilterText] = useState(value);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Sync filterText with value prop when it changes externally
  useEffect(() => {
    if (!isOpen) {
      setFilterText(value);
    }
  }, [value, isOpen]);


  // Get unique directories with metadata, sorted by recent usage
  const directoryOptions = useMemo(() => {
    if (!sessions) return [];

    // Group sessions by directory and collect metadata
    const directoryMap = sessions.reduce<Record<string, DirectoryOption>>((acc, session) => {
      const dir = session.working_directory;
      if (!acc[dir]) {
        acc[dir] = {
          path: dir,
          lastUsed: null,
          sessionCount: 0
        };
      }
      
      acc[dir].sessionCount++;
      
      // Track most recent usage
      const sessionDate = session.latest_message_date || session.earliest_message_date || null;
      if (sessionDate && (!acc[dir].lastUsed || sessionDate > acc[dir].lastUsed)) {
        acc[dir].lastUsed = sessionDate;
      }
      
      return acc;
    }, {});

    // Convert to array and sort by last used (most recent first), then by session count
    return Object.values(directoryMap).sort((a, b) => {
      // First sort by whether they have been used (used directories first)
      if (a.lastUsed && !b.lastUsed) return -1;
      if (!a.lastUsed && b.lastUsed) return 1;
      if (!a.lastUsed && !b.lastUsed) {
        // If neither has been used, sort by session count
        return b.sessionCount - a.sessionCount;
      }
      
      // Both have been used, sort by last used date
      return (b.lastUsed || '').localeCompare(a.lastUsed || '');
    });
  }, [sessions]);

  // Filter directories based on fuzzy matching
  const filteredOptions = useMemo(() => {
    if (!filterText.trim()) return directoryOptions;

    const searchTerm = filterText.toLowerCase();
    return directoryOptions.filter(option => {
      const path = option.path.toLowerCase();
      
      // Simple fuzzy matching: check if all characters of searchTerm appear in order in path
      let searchIndex = 0;
      for (let i = 0; i < path.length && searchIndex < searchTerm.length; i++) {
        if (path[i] === searchTerm[searchIndex]) {
          searchIndex++;
        }
      }
      
      return searchIndex === searchTerm.length || path.includes(searchTerm);
    }).slice(0, 10); // Limit to 10 results for performance
  }, [directoryOptions, filterText]);

  // Handle input focus
  const handleFocus = () => {
    setIsOpen(true);
    setFilterText(value);
    setSelectedIndex(-1);
  };

  // Handle input change
  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    setFilterText(newValue);
    onChange(newValue);
    setSelectedIndex(-1);
    if (!isOpen) setIsOpen(true);
  };

  // Handle input blur - accept the typed value
  const handleBlur = () => {
    const trimmedValue = filterText.trim();
    if (trimmedValue && trimmedValue !== value) {
      onChange(trimmedValue);
    }
    setIsOpen(false);
  };

  // Handle option selection
  const selectOption = (directory: string) => {
    setFilterText(directory);
    onChange(directory);
    setIsOpen(false);
    setSelectedIndex(-1);
    if (inputRef.current) {
      inputRef.current.blur();
    }
  };

  // Handle keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!isOpen) {
      if (e.key === 'ArrowDown' || e.key === 'Enter') {
        setIsOpen(true);
        setFilterText(value);
        return;
      }
    }

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setSelectedIndex(prev => 
          prev < filteredOptions.length - 1 ? prev + 1 : prev
        );
        break;
      case 'ArrowUp':
        e.preventDefault();
        setSelectedIndex(prev => prev > 0 ? prev - 1 : -1);
        break;
      case 'Enter':
        e.preventDefault();
        if (selectedIndex >= 0 && selectedIndex < filteredOptions.length) {
          selectOption(filteredOptions[selectedIndex].path);
        } else {
          // If no option is selected, use the current input value as the directory
          const trimmedValue = filterText.trim();
          if (trimmedValue) {
            onChange(trimmedValue);
          }
          setIsOpen(false);
          if (inputRef.current) {
            inputRef.current.blur();
          }
        }
        break;
      case 'Escape':
        setIsOpen(false);
        setFilterText(value);
        if (inputRef.current) {
          inputRef.current.blur();
        }
        break;
    }
  };

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current && 
        !dropdownRef.current.contains(event.target as Node) &&
        !inputRef.current?.contains(event.target as Node)
      ) {
        // Accept the typed value when clicking outside
        const trimmedValue = filterText.trim();
        if (trimmedValue && trimmedValue !== value) {
          onChange(trimmedValue);
        } else {
          setFilterText(value);
        }
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [value, filterText, onChange]);

  // Auto-scroll selected option into view
  useEffect(() => {
    if (selectedIndex >= 0 && dropdownRef.current) {
      const selectedElement = dropdownRef.current.children[selectedIndex] as HTMLElement;
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: 'nearest' });
      }
    }
  }, [selectedIndex]);

  const formatDate = (dateStr: string | null) => {
    if (!dateStr) return '';
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    
    if (diffDays === 0) {
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } else if (diffDays === 1) {
      return 'Yesterday';
    } else if (diffDays < 7) {
      return date.toLocaleDateString([], { weekday: 'short' });
    } else {
      return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
    }
  };

  return (
    <div className={`directory-picker ${className}`}>
      <input
        ref={inputRef}
        type="text"
        value={filterText}
        onChange={handleInputChange}
        onFocus={handleFocus}
        onBlur={handleBlur}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        className="directory-picker-input"
        spellCheck={false}
        autoComplete="off"
      />
      
      {isOpen && (
        <div ref={dropdownRef} className="directory-picker-dropdown">
          {filteredOptions.length === 0 ? (
            <div className="directory-picker-option empty">
              {filterText ? `No directories match "${filterText}"` : 'No recent directories found'}
            </div>
          ) : (
            filteredOptions.map((option, index) => (
              <div
                key={option.path}
                className={`directory-picker-option ${index === selectedIndex ? 'selected' : ''}`}
                onMouseDown={(e) => {
                  e.preventDefault(); // Prevent the blur event
                  selectOption(option.path);
                }}
                onMouseEnter={() => setSelectedIndex(index)}
              >
                <div className="directory-option-path">
                  <span className="directory-icon">📁</span>
                  <span className="path-text">{option.path}</span>
                </div>
                <div className="directory-option-meta">
                  <span className="session-count">{option.sessionCount} session{option.sessionCount !== 1 ? 's' : ''}</span>
                  {option.lastUsed && (
                    <span className="last-used">{formatDate(option.lastUsed)}</span>
                  )}
                </div>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}