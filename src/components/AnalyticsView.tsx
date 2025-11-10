import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { logger } from "../utils/logger";
import "./AnalyticsView.css";

interface AnalyticsStats {
  total_searches: number;
  avg_latency_ms: number;
  keyword_hit_rate: number;
  semantic_hit_rate: number;
  popular_queries: [string, number][];
}

export default function AnalyticsView() {
  const [stats, setStats] = useState<AnalyticsStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [timeRange, setTimeRange] = useState(7); // Default 7 days

  useEffect(() => {
    loadStats();
  }, [timeRange]);

  const loadStats = async () => {
    setLoading(true);
    try {
      const data = await invoke<AnalyticsStats>("get_search_stats", { days: timeRange });
      setStats(data);
      logger.info("Analytics stats loaded successfully");
    } catch (err) {
      logger.error("Failed to load analytics stats", err);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="analytics-view">
        <div className="analytics-loading">Loading analytics...</div>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="analytics-view">
        <div className="analytics-error">Failed to load analytics data</div>
      </div>
    );
  }

  return (
    <div className="analytics-view">
      <div className="analytics-header">
        <h1>Analytics Dashboard</h1>
        <div className="time-range-selector">
          <label>Time Range:</label>
          <select value={timeRange} onChange={(e) => setTimeRange(Number(e.target.value))}>
            <option value={1}>Last 24 Hours</option>
            <option value={7}>Last 7 Days</option>
            <option value={30}>Last 30 Days</option>
            <option value={90}>Last 90 Days</option>
          </select>
        </div>
      </div>

      <div className="analytics-content">
        {/* Overview Stats */}
        <section className="analytics-section">
          <h2>Overview</h2>
          <div className="stats-grid">
            <div className="stat-card">
              <div className="stat-icon">🔍</div>
              <div className="stat-content">
                <div className="stat-label">Total Searches</div>
                <div className="stat-value">{stats.total_searches.toLocaleString()}</div>
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-icon">⚡</div>
              <div className="stat-content">
                <div className="stat-label">Avg Latency</div>
                <div className="stat-value">{stats.avg_latency_ms.toFixed(0)} ms</div>
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-icon">📝</div>
              <div className="stat-content">
                <div className="stat-label">Keyword Hit Rate</div>
                <div className="stat-value">{(stats.keyword_hit_rate * 100).toFixed(1)}%</div>
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-icon">🤖</div>
              <div className="stat-content">
                <div className="stat-label">Semantic Hit Rate</div>
                <div className="stat-value">{(stats.semantic_hit_rate * 100).toFixed(1)}%</div>
              </div>
            </div>
          </div>
        </section>

        {/* Search Performance */}
        <section className="analytics-section">
          <h2>Search Performance</h2>
          <div className="performance-chart">
            <div className="performance-bars">
              <div className="performance-bar-item">
                <div className="bar-label">Keyword</div>
                <div className="bar-container">
                  <div
                    className="bar keyword-bar"
                    style={{ width: `${stats.keyword_hit_rate * 100}%` }}
                  >
                    <span className="bar-value">{(stats.keyword_hit_rate * 100).toFixed(1)}%</span>
                  </div>
                </div>
              </div>
              <div className="performance-bar-item">
                <div className="bar-label">Semantic</div>
                <div className="bar-container">
                  <div
                    className="bar semantic-bar"
                    style={{ width: `${stats.semantic_hit_rate * 100}%` }}
                  >
                    <span className="bar-value">{(stats.semantic_hit_rate * 100).toFixed(1)}%</span>
                  </div>
                </div>
              </div>
            </div>
            <div className="performance-info">
              <p>
                <strong>Keyword Search:</strong> Fast, exact matching based on text content
              </p>
              <p>
                <strong>Semantic Search:</strong> AI-powered meaning-based search using embeddings
              </p>
            </div>
          </div>
        </section>

        {/* Popular Queries */}
        <section className="analytics-section">
          <h2>Most Popular Searches</h2>
          {stats.popular_queries.length > 0 ? (
            <div className="popular-queries">
              {stats.popular_queries.map(([query, count], index) => (
                <div key={index} className="query-item">
                  <div className="query-rank">#{index + 1}</div>
                  <div className="query-text">{query}</div>
                  <div className="query-count">
                    {count} {count === 1 ? "search" : "searches"}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="no-data">No search data available for the selected time range</div>
          )}
        </section>

        {/* Performance Insights */}
        <section className="analytics-section">
          <h2>Performance Insights</h2>
          <div className="insights">
            <div className={`insight-card ${stats.avg_latency_ms < 100 ? "good" : stats.avg_latency_ms < 500 ? "warning" : "poor"}`}>
              <h3>Search Speed</h3>
              <p>
                {stats.avg_latency_ms < 100
                  ? "✅ Excellent - Your searches are lightning fast!"
                  : stats.avg_latency_ms < 500
                  ? "⚠️ Good - Search speed is acceptable but could be optimized"
                  : "❌ Needs Improvement - Consider optimizing your database"}
              </p>
            </div>
            <div className={`insight-card ${stats.semantic_hit_rate > 0.5 ? "good" : "warning"}`}>
              <h3>Semantic Search Usage</h3>
              <p>
                {stats.semantic_hit_rate > 0.5
                  ? "✅ Great - Your embeddings are helping find relevant results"
                  : "⚠️ Low Usage - Your semantic search may need more time to index content"}
              </p>
            </div>
            <div className={`insight-card ${stats.total_searches > 10 ? "good" : "warning"}`}>
              <h3>Search Activity</h3>
              <p>
                {stats.total_searches > 10
                  ? `✅ Active - You've performed ${stats.total_searches} searches`
                  : "⚠️ Low Activity - Start using search to find your snippets faster"}
              </p>
            </div>
          </div>
        </section>

        {/* Data Info */}
        <section className="analytics-section">
          <div className="info-note">
            <strong>Note:</strong> Analytics data is collected locally on your device. No data is sent to external servers.
            Search analytics help you understand your usage patterns and optimize your workflow.
          </div>
        </section>
      </div>
    </div>
  );
}
