import { useState, useEffect, useCallback } from 'react';
import type { PermissionMode } from '@anthropic-ai/claude-code/sdk';

interface ModeSwitcherProps {
  currentMode: PermissionMode;
  onModeChange: (mode: PermissionMode) => void;
}

const modeLabels: Record<PermissionMode, string> = {
  default: 'Default',
  acceptEdits: 'Accept Edits',
  plan: 'Plan',
  bypassPermissions: 'Bypass'
};

export function ModeSwitcher({ currentMode, onModeChange }: ModeSwitcherProps) {
  const [mode, setMode] = useState<PermissionMode>(currentMode);

  useEffect(() => {
    setMode(currentMode);
  }, [currentMode]);

  const cycleMode = useCallback(() => {
    const modes: PermissionMode[] = ['default', 'acceptEdits', 'plan'];
    const currentIndex = modes.indexOf(mode);
    const nextIndex = (currentIndex + 1) % modes.length;
    const nextMode = modes[nextIndex];
    setMode(nextMode);
    onModeChange(nextMode);
  }, [mode, onModeChange]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.shiftKey && e.key === 'Tab') {
        e.preventDefault();
        cycleMode();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [cycleMode]);

  return (
    <button
      className={`mode-switcher mode-${mode}`}
      onClick={cycleMode}
      title={`Current mode: ${modeLabels[mode]}. Click or press Shift+Tab to cycle modes.`}
    >
      {modeLabels[mode]}
    </button>
  );
}