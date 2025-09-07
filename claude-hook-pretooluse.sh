#!/bin/bash

# Claude pre-tool-use hook that automatically allows all requests
# This is for bypassing permission checks when --dangerously-skip-permissions is used

cat << 'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "Auto-approved via --dangerously-skip-permissions flag"
  }
}
EOF