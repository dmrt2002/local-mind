#!/bin/bash

# LocalMind CLI Bridge
# Queries saved commands from the LocalMind database for terminal autocomplete

# Get database path (relative to script location or current directory)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_PATH="${SCRIPT_DIR}/data/local-mind/snippets.db"

# Fallback to current directory if script path doesn't work
if [ ! -f "$DB_PATH" ]; then
    DB_PATH="$(pwd)/data/local-mind/snippets.db"
fi

# Check if database exists
if [ ! -f "$DB_PATH" ]; then
    echo "Error: LocalMind database not found at $DB_PATH" >&2
    exit 1
fi

# Check if sqlite3 is available
if ! command -v sqlite3 &> /dev/null; then
    echo "Error: sqlite3 not found. Please install sqlite3." >&2
    exit 1
fi

# Parse arguments
SEARCH=""
LIMIT=100
CWD=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --search)
            SEARCH="$2"
            shift 2
            ;;
        --limit)
            LIMIT="$2"
            shift 2
            ;;
        --cwd)
            CWD="$2"
            shift 2
            ;;
        *)
            echo "Usage: $0 [--search TERM] [--limit N] [--cwd PATH]" >&2
            exit 1
            ;;
    esac
done

# Build SQL query
if [ -n "$SEARCH" ] && [ -n "$CWD" ]; then
    # Search with both search term and working directory
    QUERY="SELECT content FROM snippets WHERE type = 'command' AND (content LIKE '%${SEARCH}%' OR summary LIKE '%${SEARCH}%') AND working_directory = '${CWD}' ORDER BY created_at DESC LIMIT ${LIMIT};"
elif [ -n "$SEARCH" ]; then
    # Search with search term only
    QUERY="SELECT content FROM snippets WHERE type = 'command' AND (content LIKE '%${SEARCH}%' OR summary LIKE '%${SEARCH}%') ORDER BY created_at DESC LIMIT ${LIMIT};"
elif [ -n "$CWD" ]; then
    # Filter by working directory only
    QUERY="SELECT content FROM snippets WHERE type = 'command' AND working_directory = '${CWD}' ORDER BY created_at DESC LIMIT ${LIMIT};"
else
    # Get all commands
    QUERY="SELECT content FROM snippets WHERE type = 'command' ORDER BY created_at DESC LIMIT ${LIMIT};"
fi

# Execute query and output commands (one per line)
sqlite3 -noheader -separator $'\n' "$DB_PATH" "$QUERY" 2>/dev/null

