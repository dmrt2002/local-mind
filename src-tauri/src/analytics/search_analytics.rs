use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchType {
    Keyword,
    Semantic,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMetrics {
    pub query: String,
    pub search_type: SearchType,
    pub keyword_results: usize,
    pub semantic_results: usize,
    pub total_results: usize,
    pub latency_ms: u64,
    pub timestamp: String,
}

pub struct SearchAnalytics {
    pool: SqlitePool,
}

impl SearchAnalytics {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        let analytics = Self { pool };
        analytics.init_table().await?;
        Ok(analytics)
    }

    async fn init_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS search_analytics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                search_type TEXT NOT NULL,
                keyword_results INTEGER NOT NULL,
                semantic_results INTEGER NOT NULL,
                total_results INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                timestamp TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create search_analytics table")?;

        // Create index for faster queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_search_analytics_timestamp
            ON search_analytics(timestamp)
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create timestamp index")?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_search_analytics_query
            ON search_analytics(query)
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create query index")?;

        Ok(())
    }

    /// Log a search event
    pub async fn log_search(&self, metrics: SearchMetrics) -> Result<()> {
        let search_type_str = match metrics.search_type {
            SearchType::Keyword => "keyword",
            SearchType::Semantic => "semantic",
            SearchType::Hybrid => "hybrid",
        };

        sqlx::query(
            r#"
            INSERT INTO search_analytics
            (query, search_type, keyword_results, semantic_results, total_results, latency_ms, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&metrics.query)
        .bind(search_type_str)
        .bind(metrics.keyword_results as i64)
        .bind(metrics.semantic_results as i64)
        .bind(metrics.total_results as i64)
        .bind(metrics.latency_ms as i64)
        .bind(&metrics.timestamp)
        .execute(&self.pool)
        .await
        .context("Failed to log search event")?;

        log::debug!(
            "📊 Logged search: query='{}', type={}, results={}, latency={}ms",
            metrics.query,
            search_type_str,
            metrics.total_results,
            metrics.latency_ms
        );

        Ok(())
    }

    /// Get search statistics for the last N days
    pub async fn get_stats(&self, days: i64) -> Result<Stats> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();

        // Total searches
        let total_searches: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM search_analytics WHERE timestamp >= ?"
        )
        .bind(&cutoff_str)
        .fetch_one(&self.pool)
        .await?;

        // Average latency
        let avg_latency: f64 = sqlx::query_scalar(
            "SELECT AVG(latency_ms) FROM search_analytics WHERE timestamp >= ?"
        )
        .bind(&cutoff_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0.0);

        // Keyword vs semantic hit rates
        let keyword_hits: i64 = sqlx::query_scalar(
            "SELECT SUM(keyword_results) FROM search_analytics WHERE timestamp >= ?"
        )
        .bind(&cutoff_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        let semantic_hits: i64 = sqlx::query_scalar(
            "SELECT SUM(semantic_results) FROM search_analytics WHERE timestamp >= ?"
        )
        .bind(&cutoff_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        // Popular queries
        let popular: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT query, COUNT(*) as count
            FROM search_analytics
            WHERE timestamp >= ?
            GROUP BY query
            ORDER BY count DESC
            LIMIT 10
            "#
        )
        .bind(&cutoff_str)
        .fetch_all(&self.pool)
        .await?;

        Ok(Stats {
            total_searches,
            avg_latency_ms: avg_latency,
            keyword_hit_rate: if total_searches > 0 {
                (keyword_hits as f64) / (total_searches as f64)
            } else {
                0.0
            },
            semantic_hit_rate: if total_searches > 0 {
                (semantic_hits as f64) / (total_searches as f64)
            } else {
                0.0
            },
            popular_queries: popular,
        })
    }

    /// Clean up old analytics data
    pub async fn cleanup_old_data(&self, days: i64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();

        let result = sqlx::query(
            "DELETE FROM search_analytics WHERE timestamp < ?"
        )
        .bind(&cutoff_str)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected() as usize;

        if deleted > 0 {
            log::info!("Cleaned up {} old analytics records", deleted);
            println!("🧹 Cleaned up {} old analytics records", deleted);
        }

        Ok(deleted)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stats {
    pub total_searches: i64,
    pub avg_latency_ms: f64,
    pub keyword_hit_rate: f64,
    pub semantic_hit_rate: f64,
    pub popular_queries: Vec<(String, i64)>,
}
