# Search Fix - Issue Resolution

## Problem
Search was returning no results because:
1. The FTS5 formatted query (with `AND`, `*`, etc.) was being used for validation
2. Validation was looking for words like `"gre*"` and `"AND"` in content, which don't exist

## Solution
1. Clean the query before validation by removing FTS5 operators
2. Extract actual search terms for validation
3. Use original query terms, not the formatted FTS5 query

## Changes Made

### `src-tauri/src/db/sqlite.rs`
- Updated validation to extract clean search terms from query
- Removes FTS5 operators (`AND`, `OR`, `NOT`, `NEAR/`, `*`, quotes) before validation
- Filters out operator keywords from word list

### `src-tauri/src/commands.rs`
- Added logging to debug query parsing
- Passes both formatted and original query to search function

## Testing
After restarting the dev server, searches should work:
- Basic: `greensage`
- Prefix: `gre` (matches greensage)
- Phrase: `"greensage ai"`
- Boolean: `greensage AND algorithm`

