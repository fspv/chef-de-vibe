import { useState } from 'react';
import type { PermissionMode } from '@anthropic-ai/claude-code/sdk';
import './ModeSelector.css';

interface ModeSelectorProps {
  value: PermissionMode;
  onChange: (mode: PermissionMode) => void;
}

interface ModeOption {
  value: PermissionMode;
  label: string;
  color: string;
  description: string;
}

const modes: ModeOption[] = [
  {
    value: 'default',
    label: 'Default',
    color: '#6c757d',
    description: 'Standard mode with normal permissions'
  },
  {
    value: 'acceptEdits',
    label: 'Accept Edits',
    color: '#28a745',
    description: 'Automatically accept all edit requests'
  },
  {
    value: 'plan',
    label: 'Plan',
    color: '#17a2b8',
    description: 'Planning mode for complex tasks'
  }
];

export function ModeSelector({ value, onChange }: ModeSelectorProps) {
  const [selectedMode, setSelectedMode] = useState<PermissionMode>(value);

  const handleModeChange = (mode: PermissionMode) => {
    setSelectedMode(mode);
    onChange(mode);
  };

  return (
    <div className="mode-selector-container">
      <div className="mode-selector-bar">
        {modes.map((mode) => (
          <button
            key={mode.value}
            className={`mode-selector-segment ${selectedMode === mode.value ? 'active' : ''}`}
            onClick={() => handleModeChange(mode.value)}
            style={{
              '--segment-color': mode.color
            } as React.CSSProperties}
            title={mode.description}
          >
            <span className="mode-label">{mode.label}</span>
          </button>
        ))}
      </div>
      <div className="mode-description">
        {modes.find(m => m.value === selectedMode)?.description}
      </div>
    </div>
  );
}