import type {
  SessionsResponse,
  CreateSessionRequest,
  CreateSessionResponse,
  SessionDetailsResponse,
  ApiResponse,
} from '../types/api';
import { isApiError } from '../types/api';

// Dynamic API base URL that adapts to the current frontend location
const getApiBaseUrl = (): string => {
  // Try environment variable first (for development)
  const envUrl = import.meta.env.VITE_API_BASE_URL;
  if (envUrl) {
    return envUrl;
  }
  
  // Default: construct backend URL from current frontend location
  const { protocol, hostname, port } = window.location;
  
  return `${protocol}//${hostname}${port ? `:${port}` : ''}`;
};

const API_BASE_URL = getApiBaseUrl();

class ApiError extends Error {
  constructor(public code: string, message: string) {
    super(message);
    this.name = 'ApiError';
  }
}

async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
    ...options,
  });

  if (!response.ok) {
    throw new ApiError('HTTP_ERROR', `HTTP ${response.status}: ${response.statusText}`);
  }

  const data: ApiResponse<T> = await response.json();
  
  if (isApiError(data)) {
    throw new ApiError(data.code, data.error);
  }

  return data;
}

export const api = {
  async getSessions(): Promise<SessionsResponse> {
    return fetchJson<SessionsResponse>(`${API_BASE_URL}/api/v1/sessions`);
  },

  async getSessionDetails(sessionId: string): Promise<SessionDetailsResponse> {
    return fetchJson<SessionDetailsResponse>(`${API_BASE_URL}/api/v1/sessions/${sessionId}`);
  },

  async createSession(request: CreateSessionRequest): Promise<CreateSessionResponse> {
    return fetchJson<CreateSessionResponse>(`${API_BASE_URL}/api/v1/sessions`, {
      method: 'POST',
      body: JSON.stringify(request),
    });
  },

  buildWebSocketUrl(path: string): string {
    // Try environment variable first (for development)
    const envWsUrl = import.meta.env.VITE_WS_BASE_URL;
    if (envWsUrl) {
      return `${envWsUrl}${path}`;
    }
    
    // Default: construct WebSocket URL from current frontend location
    const { protocol, hostname, port } = window.location;
    const wsProtocol = protocol === 'https:' ? 'wss:' : 'ws:';
    
    return `${wsProtocol}//${hostname}${port ? `:${port}` : ''}${path}`;
  },
};

export { ApiError };
