import type { Session } from '../types/api';
import { api } from './api';

interface DirectoryOption {
  path: string;
  lastUsed: string | null;
  sessionCount: number;
}

interface CacheData {
  directories: DirectoryOption[];
  timestamp: number;
}

const CACHE_KEY = 'chef-de-vibe-directory-cache';
const CACHE_TTL = 5 * 60 * 1000; // 5 minutes

class DirectoryCache {
  private cache: CacheData | null = null;
  private isRefreshing = false;
  private refreshPromise: Promise<DirectoryOption[]> | null = null;
  private subscribers: Set<(directories: DirectoryOption[]) => void> = new Set();

  constructor() {
    this.loadFromLocalStorage();
  }

  private loadFromLocalStorage(): void {
    try {
      // Check if localStorage is available (might not be in private mode or some browsers)
      if (typeof localStorage === 'undefined' || !localStorage) {
        return;
      }
      const stored = localStorage.getItem(CACHE_KEY);
      if (stored) {
        const data: CacheData = JSON.parse(stored);
        // Check if cache is not too old (optional, could always use stale cache)
        if (Date.now() - data.timestamp < CACHE_TTL * 2) {
          this.cache = data;
        }
      }
    } catch (error) {
      console.warn('Failed to load directory cache from localStorage:', error);
      // Clear cache on error to prevent issues
      this.cache = null;
    }
  }

  private saveToLocalStorage(directories: DirectoryOption[]): void {
    try {
      // Check if localStorage is available
      if (typeof localStorage === 'undefined' || !localStorage) {
        // Still update in-memory cache
        this.cache = {
          directories,
          timestamp: Date.now()
        };
        return;
      }
      const data: CacheData = {
        directories,
        timestamp: Date.now()
      };
      localStorage.setItem(CACHE_KEY, JSON.stringify(data));
      this.cache = data;
    } catch (error) {
      console.warn('Failed to save directory cache to localStorage:', error);
      // Still update in-memory cache even if localStorage fails
      this.cache = {
        directories,
        timestamp: Date.now()
      };
    }
  }

  private processSessionsToDirectories(sessions: Session[]): DirectoryOption[] {
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
  }

  /**
   * Get cached directories immediately if available, otherwise return empty array
   */
  getCachedDirectories(): DirectoryOption[] {
    return this.cache?.directories || [];
  }

  /**
   * Check if cache is expired
   */
  isCacheExpired(): boolean {
    if (!this.cache) return true;
    return Date.now() - this.cache.timestamp > CACHE_TTL;
  }

  /**
   * Fetch directories from API and update cache
   */
  async fetchDirectories(): Promise<DirectoryOption[]> {
    try {
      const response = await api.getSessions();
      const directories = this.processSessionsToDirectories(response.sessions);
      this.saveToLocalStorage(directories);
      return directories;
    } catch (error) {
      console.error('Failed to fetch directories:', error);
      // Return cached data if fetch fails
      return this.getCachedDirectories();
    }
  }

  /**
   * Get directories with background refresh if needed
   * Returns cached data immediately and triggers background refresh if expired
   */
  async getDirectoriesWithBackgroundRefresh(): Promise<{
    directories: DirectoryOption[];
    isFromCache: boolean;
    refreshPromise?: Promise<DirectoryOption[]>;
  }> {
    const cachedDirectories = this.getCachedDirectories();
    const isExpired = this.isCacheExpired();

    // If cache is expired and not already refreshing, start background refresh
    if (isExpired && !this.isRefreshing) {
      this.isRefreshing = true;
      this.refreshPromise = this.fetchDirectories()
        .then(directories => {
          // Notify subscribers about the update
          this.subscribers.forEach(callback => callback(directories));
          return directories;
        })
        .finally(() => {
          this.isRefreshing = false;
          this.refreshPromise = null;
        });
    }

    return {
      directories: cachedDirectories,
      isFromCache: cachedDirectories.length > 0,
      refreshPromise: this.refreshPromise || undefined
    };
  }

  /**
   * Subscribe to directory updates
   */
  subscribe(callback: (directories: DirectoryOption[]) => void): () => void {
    this.subscribers.add(callback);
    return () => {
      this.subscribers.delete(callback);
    };
  }

  /**
   * Force refresh the cache
   */
  async forceRefresh(): Promise<DirectoryOption[]> {
    return this.fetchDirectories();
  }

  /**
   * Clear the cache
   */
  clearCache(): void {
    this.cache = null;
    try {
      if (typeof localStorage !== 'undefined' && localStorage) {
        localStorage.removeItem(CACHE_KEY);
      }
    } catch (error) {
      console.warn('Failed to clear directory cache:', error);
    }
  }
}

// Export singleton instance
export const directoryCache = new DirectoryCache();
export type { DirectoryOption };