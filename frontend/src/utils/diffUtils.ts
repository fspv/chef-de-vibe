interface EditToolData {
  file_path: string;
  old_string: string;
  new_string: string;
  replace_all?: boolean;
}

// Helper function to detect if tool usage is an Edit tool
export function isEditTool(toolName: string, toolInput: unknown): toolInput is EditToolData {
  return toolName === 'Edit' && 
         typeof toolInput === 'object' &&
         toolInput !== null &&
         'file_path' in toolInput &&
         'old_string' in toolInput &&
         'new_string' in toolInput;
}

// Function to detect language from file extension
export function getLanguageFromFilePath(filePath: string): string {
  const extension = filePath.split('.').pop()?.toLowerCase();
  
  const extensionMap: Record<string, string> = {
    'js': 'javascript',
    'jsx': 'jsx',
    'ts': 'typescript',
    'tsx': 'tsx',
    'py': 'python',
    'rs': 'rust',
    'go': 'go',
    'java': 'java',
    'cpp': 'cpp',
    'c': 'c',
    'cs': 'csharp',
    'php': 'php',
    'rb': 'ruby',
    'swift': 'swift',
    'kt': 'kotlin',
    'scala': 'scala',
    'css': 'css',
    'scss': 'scss',
    'html': 'html',
    'xml': 'xml',
    'json': 'json',
    'yaml': 'yaml',
    'yml': 'yaml',
    'md': 'markdown',
    'sh': 'bash',
    'sql': 'sql',
    'r': 'r',
    'dart': 'dart',
    'vim': 'vim'
  };
  
  return extensionMap[extension || ''] || 'text';
}

export type { EditToolData };