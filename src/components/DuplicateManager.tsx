import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { logger } from "../utils/logger";
import { useToast } from "../hooks/useToast";
import "./DuplicateManager.css";

interface DuplicateSnippet {
  id: number;
  content: string;
  created_at: string;
  source_app: string | null;
}

interface DuplicateGroup {
  hash: string;
  snippets: DuplicateSnippet[];
  count: number;
}

interface DuplicateStats {
  total_snippets: number;
  unique_snippets: number;
  duplicate_snippets: number;
  duplicate_groups: number;
  duplicate_rate: number;
}

export default function DuplicateManager() {
  const [stats, setStats] = useState<DuplicateStats | null>(null);
  const [duplicates, setDuplicates] = useState<DuplicateGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [merging, setMerging] = useState<string | null>(null);
  const { showToast } = useToast();

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      await Promise.all([loadStats(), loadDuplicates()]);
    } catch (err) {
      logger.error("Failed to load duplicate data", err);
      showToast("Failed to load duplicate data", "error");
    } finally {
      setLoading(false);
    }
  };

  const loadStats = async () => {
    try {
      const data = await invoke<DuplicateStats>("get_duplicate_stats");
      setStats(data);
    } catch (err) {
      logger.error("Failed to load duplicate stats", err);
    }
  };

  const loadDuplicates = async () => {
    try {
      const data = await invoke<DuplicateGroup[]>("find_all_duplicates");
      setDuplicates(data);
    } catch (err) {
      logger.error("Failed to load duplicates", err);
    }
  };

  const handleScan = async () => {
    setScanning(true);
    try {
      const updated = await invoke<number>("update_all_content_hashes");
      showToast(`Updated hashes for ${updated} snippets`, "success");
      await loadData();
    } catch (err) {
      logger.error("Failed to scan for duplicates", err);
      showToast("Failed to scan for duplicates", "error");
    } finally {
      setScanning(false);
    }
  };

  const handleMerge = async (hash: string) => {
    setMerging(hash);
    try {
      const deleted = await invoke<number>("merge_duplicate_group", { hash });
      showToast(`Merged ${deleted} duplicate(s)`, "success");
      await loadData();
    } catch (err) {
      logger.error("Failed to merge duplicates", err);
      showToast("Failed to merge duplicates", "error");
    } finally {
      setMerging(null);
    }
  };

  const handleDeleteSnippet = async (snippetId: number, hash: string) => {
    try {
      await invoke("delete_snippet", { id: snippetId });
      showToast("Snippet deleted", "success");
      await loadData();
    } catch (err) {
      logger.error("Failed to delete snippet", err);
      showToast("Failed to delete snippet", "error");
    }
  };

  const formatDate = (dateStr: string): string => {
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return dateStr;
    }
  };

  const truncateContent = (content: string, maxLength: number = 100): string => {
    if (content.length <= maxLength) return content;
    return content.substring(0, maxLength) + "...";
  };

  if (loading) {
    return (
      <div className="duplicate-manager">
        <div className="loading">Loading duplicate data...</div>
      </div>
    );
  }

  return (
    <div className="duplicate-manager">
      <div className="duplicate-header">
        <h2>Duplicate Detection</h2>
        <button
          className="scan-button"
          onClick={handleScan}
          disabled={scanning}
        >
          {scanning ? "Scanning..." : "Scan for Duplicates"}
        </button>
      </div>

      {/* Stats Section */}
      {stats && (
        <div className="duplicate-stats">
          <div className="stats-grid">
            <div className="stat-card">
              <div className="stat-label">Total Snippets</div>
              <div className="stat-value">{stats.total_snippets}</div>
            </div>
            <div className="stat-card">
              <div className="stat-label">Unique Snippets</div>
              <div className="stat-value">{stats.unique_snippets}</div>
            </div>
            <div className="stat-card">
              <div className="stat-label">Duplicate Snippets</div>
              <div className="stat-value duplicate-count">
                {stats.duplicate_snippets}
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-label">Duplicate Groups</div>
              <div className="stat-value">{stats.duplicate_groups}</div>
            </div>
          </div>

          <div className="duplicate-rate-bar">
            <div className="rate-label">Duplicate Rate</div>
            <div className="rate-bar-container">
              <div
                className={`rate-bar ${stats.duplicate_rate > 0.1 ? "high" : "low"}`}
                style={{ width: `${stats.duplicate_rate * 100}%` }}
              >
                <span className="rate-value">
                  {(stats.duplicate_rate * 100).toFixed(1)}%
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Duplicates List */}
      <div className="duplicates-section">
        <h3>Duplicate Groups ({duplicates.length})</h3>
        {duplicates.length === 0 ? (
          <div className="no-duplicates">
            <div className="no-duplicates-icon">✓</div>
            <div className="no-duplicates-text">
              No duplicates found! Your snippets are clean.
            </div>
          </div>
        ) : (
          <div className="duplicate-groups">
            {duplicates.map((group) => (
              <div key={group.hash} className="duplicate-group">
                <div className="group-header">
                  <div className="group-info">
                    <span className="group-count">{group.count} duplicates</span>
                    <span className="group-hash">{group.hash.substring(0, 8)}...</span>
                  </div>
                  <button
                    className="merge-button"
                    onClick={() => handleMerge(group.hash)}
                    disabled={merging === group.hash}
                  >
                    {merging === group.hash
                      ? "Merging..."
                      : "Merge (Keep Oldest)"}
                  </button>
                </div>

                <div className="group-snippets">
                  {group.snippets.map((snippet, index) => (
                    <div
                      key={snippet.id}
                      className={`snippet-item ${index === 0 ? "oldest" : ""}`}
                    >
                      <div className="snippet-content-preview">
                        <div className="snippet-meta">
                          <span className="snippet-id">#{snippet.id}</span>
                          {index === 0 && (
                            <span className="oldest-badge">Oldest (Will be kept)</span>
                          )}
                          <span className="snippet-date">
                            {formatDate(snippet.created_at)}
                          </span>
                          {snippet.source_app && (
                            <span className="snippet-source">{snippet.source_app}</span>
                          )}
                        </div>
                        <div className="snippet-text">
                          {truncateContent(snippet.content)}
                        </div>
                      </div>
                      <button
                        className="delete-snippet-button"
                        onClick={() => handleDeleteSnippet(snippet.id, group.hash)}
                        title="Delete this snippet"
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="info-note">
        <strong>How it works:</strong> Duplicate detection uses SHA-256 hashing to identify
        exact content matches. When merging, the oldest snippet is kept and newer duplicates
        are deleted. This helps keep your database clean and organized.
      </div>
    </div>
  );
}
