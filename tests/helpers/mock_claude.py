#!/usr/bin/env python3
"""
Minimal mock Claude binary for testing.

By default, echoes back any JSON input it receives.
Supports control commands in JSON format:
  - {"control": "exit", "code": 1}: Exit with specified code
  - {"control": "sleep", "duration": 1.5}: Sleep for specified duration
  - {"control": "write_file", "path": "/path/to/file", "content": "data"}: Write content to file
"""

import sys
import json
import time
import os
from pathlib import Path


def main():
    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break
            
            line = line.strip()
            
            try:
                data = json.loads(line)
                
                # Check for control commands
                if isinstance(data, dict) and "control" in data:
                    control = data["control"]
                    
                    if control == "exit":
                        code = data.get("code", 1)
                        sys.exit(code)
                    
                    elif control == "sleep":
                        duration = data.get("duration", 1.0)
                        time.sleep(duration)
                        continue
                    
                    elif control == "write_file":
                        file_path = data.get("path")
                        content = data.get("content", "")
                        if file_path:
                            try:
                                # Create parent directories if needed
                                Path(file_path).parent.mkdir(parents=True, exist_ok=True)
                                with open(file_path, 'w') as f:
                                    f.write(content)
                            except Exception as e:
                                print(json.dumps({"error": f"Failed to write file: {e}"}), flush=True)
                        continue
                
                # Echo back the JSON
                print(json.dumps(data), flush=True)
                
            except json.JSONDecodeError:
                # Should not happen as backend validates JSON, but just in case
                print(json.dumps({"error": "Invalid JSON", "input": line}), flush=True)
                
        except KeyboardInterrupt:
            break
        except Exception as e:
            # Log error but continue
            print(json.dumps({"error": str(e)}), flush=True)


if __name__ == "__main__":
    main()
