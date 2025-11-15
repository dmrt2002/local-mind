#!/bin/bash

# Migrate existing screenshots to managed directory
# This script copies existing screenshots and updates the database

DB_PATH="data/local-mind/snippets.db"
SCREENSHOTS_DIR="data/local-mind/screenshots"

echo "🔄 Migrating existing screenshots to managed directory..."

# Get all screenshot records
sqlite3 "$DB_PATH" "SELECT id, file_path, content_hash FROM snippets WHERE type = 'screenshot' AND file_path IS NOT NULL;" | while IFS='|' read -r id file_path content_hash; do
    if [ -z "$file_path" ] || [ -z "$content_hash" ]; then
        continue
    fi

    # Get extension from original file
    ext="${file_path##*.}"

    # Create new filename from hash
    new_filename="${content_hash}.${ext}"
    new_path="${SCREENSHOTS_DIR}/${new_filename}"

    # Check if file exists in managed directory
    if [ -f "$new_path" ]; then
        echo "✓ Screenshot $id already in managed directory"
        # Update database path
        sqlite3 "$DB_PATH" "UPDATE snippets SET file_path = '$PWD/$new_path' WHERE id = $id;"
        continue
    fi

    # Try to find the source file using wildcard (handles Unicode issues)
    base_name=$(basename "$file_path")
    dir_name=$(dirname "$file_path")

    # Use find with -name pattern (works better with Unicode)
    source_file=$(find "$dir_name" -maxdepth 1 -name "$base_name" -type f 2>/dev/null | head -1)

    if [ -n "$source_file" ] && [ -f "$source_file" ]; then
        # Copy to managed directory
        cp "$source_file" "$new_path"
        echo "✓ Copied screenshot $id to managed directory"

        # Update database path
        sqlite3 "$DB_PATH" "UPDATE snippets SET file_path = '$PWD/$new_path' WHERE id = $id;"
    else
        echo "⚠ Source file not found for screenshot $id: $file_path"
    fi
done

echo "✅ Migration complete!"
