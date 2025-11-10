# Database Migrations Guide

LocalMind uses a simple migration system to manage database schema changes. Migrations are automatically run on startup.

## How It Works

1. **Migration System**: Located in `src-tauri/src/db/migrations.rs`
2. **Version Tracking**: Tracks schema version in `schema_migrations` table
3. **Automatic Execution**: Runs on every app startup
4. **Idempotent**: Safe to run multiple times

## Current Migrations

### Version 1: Initial Schema
- Creates `snippets` table
- Creates `snippets_fts` FTS5 virtual table
- Sets up triggers for FTS5 sync

### Version 2: Fix FTS5 Triggers
- Fixes FTS5 table schema (removes invalid external content syntax)
- Migrates existing data to new FTS5 table
- Recreates triggers with correct syntax

## Adding New Migrations

### Step 1: Increment Version
Update `CURRENT_VERSION` in `migrations.rs`:
```rust
const CURRENT_VERSION: i32 = 3; // Increment this
```

### Step 2: Add Migration Definition
Add a new `Migration` struct to the `MIGRATIONS` array:
```rust
Migration {
    version: 3,
    name: "add_new_column",
    up: r#"
        ALTER TABLE snippets ADD COLUMN new_field TEXT;
    "#,
},
```

### Step 3: Write SQL
Write your SQL migration in the `up` field. The SQL should:
- Be idempotent (safe to run multiple times)
- Use `IF NOT EXISTS` or check before creating
- Handle data migration if needed

### Example: Adding a Column
```rust
Migration {
    version: 3,
    name: "add_tags_column",
    up: r#"
        -- Add tags column if it doesn't exist
        -- Note: SQLite doesn't support IF NOT EXISTS for ALTER TABLE
        -- So we check first or handle it in application code
        
        -- For columns, you might need to:
        -- 1. Create new table with column
        -- 2. Copy data
        -- 3. Drop old table
        -- 4. Rename new table
        
        -- Or use a different approach depending on SQLite version
    "#,
},
```

### Example: Creating an Index
```rust
Migration {
    version: 4,
    name: "add_created_at_index",
    up: r#"
        CREATE INDEX IF NOT EXISTS idx_snippets_created_at 
        ON snippets(created_at DESC);
    "#,
},
```

### Example: Data Migration
```rust
Migration {
    version: 5,
    name: "migrate_metadata_format",
    up: r#"
        -- Update existing rows
        UPDATE snippets 
        SET metadata = json_object('old', metadata)
        WHERE metadata IS NOT NULL AND metadata NOT LIKE '{%';
        
        -- Or more complex transformations
    "#,
},
```

## Migration Best Practices

1. **Always Test**: Test migrations on a copy of production data
2. **Backward Compatible**: Try to maintain backward compatibility when possible
3. **Document Changes**: Document what each migration does
4. **Rollback Plan**: Consider how to rollback (manual SQL or new migration)
5. **Data Safety**: Always backup before major migrations

## Checking Migration Status

The migration system logs to the console:
```
[INFO] Checking database migrations...
[INFO] Current database version: 1
[INFO] Found 1 pending migration(s)
[INFO] Applying migration 2: fix_fts5_triggers
[INFO] ✓ Migration 2 applied successfully
[INFO] ✓ Migrations complete! Database upgraded from version 1 to 2
```

## Manual Migration (Advanced)

If you need to manually run migrations:

```bash
# Connect to database
sqlite3 data/local-mind/snippets.db

# Check current version
SELECT * FROM schema_migrations;

# Check pending migrations
# (Compare CURRENT_VERSION in code vs database version)
```

## Troubleshooting

### Migration Fails
1. Check the error logs
2. Verify SQL syntax is correct
3. Check if migration conflicts with existing data
4. Manually fix database if needed and update `schema_migrations`

### Database Out of Sync
If migrations get out of sync:
1. Check `schema_migrations` table
2. Compare with `CURRENT_VERSION` in code
3. Manually run missing migrations or adjust version

### FTS5 Issues
If FTS5 table needs recreation:
```sql
DROP TABLE IF EXISTS snippets_fts;
-- Recreate in migration
```

## Future Improvements

- [ ] Add rollback/down migrations
- [ ] Migration validation/preview
- [ ] Migration testing utilities
- [ ] Migration rollback UI

