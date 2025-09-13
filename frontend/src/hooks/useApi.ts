import { useState, useEffect, useCallback } from 'react';
import { api, ApiError } from '../services/api';
import type { Session, SessionDetailsResponse, CreateSessionRequest } from '../types/api';

export function useSessions() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSessions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await api.getSessions();
      setSessions(response.sessions);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(`Failed to load sessions: ${err.message}`);
      } else {
        setError('Failed to load sessions: Unknown error');
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);

  return { sessions, loading, error, refetch: fetchSessions };
}

export function useSessionDetails(sessionId: string | null) {
  const [sessionDetails, setSessionDetails] = useState<SessionDetailsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchSessionDetails = useCallback(async () => {
    if (!sessionId) {
      setSessionDetails(null);
      return;
    }

    try {
      setLoading(true);
      setError(null);
      const response = await api.getSessionDetails(sessionId);
      setSessionDetails(response);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(`Failed to load session: ${err.message}`);
      } else {
        setError('Failed to load session: Unknown error');
      }
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    fetchSessionDetails();
  }, [fetchSessionDetails]);

  return { sessionDetails, loading, error, refetch: fetchSessionDetails };
}

export function useCreateSession() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const createSession = useCallback(async (request: CreateSessionRequest) => {
    try {
      setLoading(true);
      setError(null);
      const response = await api.createSession(request);
      return response;
    } catch (err) {
      if (err instanceof ApiError) {
        setError(`Failed to create session: ${err.message}`);
      } else {
        setError('Failed to create session: Unknown error');
      }
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  return { createSession, loading, error };
}