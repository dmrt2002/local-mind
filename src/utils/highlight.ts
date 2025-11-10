/**
 * Improved highlighting with matched context extraction
 */

/**
 * Extract meaningful words from search query
 */
function extractQueryWords(query: string): string[] {
  // Handle quoted phrases
  const phrases: string[] = [];
  const phraseRegex = /"([^"]+)"/g;
  let match;

  while ((match = phraseRegex.exec(query)) !== null) {
    phrases.push(match[1].toLowerCase());
  }

  // Remove quoted phrases from query
  const withoutPhrases = query.replace(phraseRegex, '');

  // Extract individual words
  const words = withoutPhrases
    .toLowerCase()
    .replace(/[()]/g, ' ')
    .split(/\s+/)
    .filter(word =>
      word.length > 2 &&
      !['and', 'or', 'not'].includes(word)
    );

  return [...phrases, ...words];
}

/**
 * Find the best matching snippet with context around matches
 */
function findBestMatchingSnippet(
  content: string,
  queryWords: string[],
  maxLength: number
): { text: string; offset: number } {
  const contentLower = content.toLowerCase();

  // Find all match positions
  const matchPositions: number[] = [];

  for (const word of queryWords) {
    let pos = 0;
    while ((pos = contentLower.indexOf(word, pos)) !== -1) {
      matchPositions.push(pos);
      pos += word.length;
    }
  }

  if (matchPositions.length === 0) {
    // No matches - return beginning
    return {
      text: content.substring(0, maxLength) + (content.length > maxLength ? '...' : ''),
      offset: 0
    };
  }

  // Sort positions
  matchPositions.sort((a, b) => a - b);

  // Find the position with highest match density
  let bestStart = 0;
  let bestScore = 0;

  for (let i = 0; i < matchPositions.length; i++) {
    const start = matchPositions[i];
    const end = start + maxLength;

    // Count matches in this window
    const matchesInWindow = matchPositions.filter(pos => pos >= start && pos < end).length;

    if (matchesInWindow > bestScore) {
      bestScore = matchesInWindow;
      bestStart = start;
    }
  }

  // Expand to word boundaries
  let snippetStart = Math.max(0, bestStart - 50); // Add context before
  let snippetEnd = Math.min(content.length, bestStart + maxLength + 50);

  // Adjust to word boundaries
  if (snippetStart > 0) {
    const spaceIdx = content.indexOf(' ', snippetStart);
    if (spaceIdx !== -1 && spaceIdx < snippetStart + 20) {
      snippetStart = spaceIdx + 1;
    }
  }

  if (snippetEnd < content.length) {
    const spaceIdx = content.lastIndexOf(' ', snippetEnd);
    if (spaceIdx !== -1 && spaceIdx > snippetEnd - 20) {
      snippetEnd = spaceIdx;
    }
  }

  let snippet = content.substring(snippetStart, snippetEnd);

  // Add ellipsis
  if (snippetStart > 0) snippet = '...' + snippet;
  if (snippetEnd < content.length) snippet = snippet + '...';

  return { text: snippet, offset: snippetStart };
}

/**
 * Highlight all occurrences of matching words in text
 */
function highlightAllMatches(text: string, queryWords: string[]): string {
  if (queryWords.length === 0) {
    return escapeHtml(text);
  }

  const textLower = text.toLowerCase();
  const matches: Array<{start: number, end: number}> = [];

  // Find all matches (phrases and words)
  for (const queryWord of queryWords) {
    const word = queryWord.toLowerCase();
    let pos = 0;

    while ((pos = textLower.indexOf(word, pos)) !== -1) {
      matches.push({
        start: pos,
        end: pos + word.length
      });
      pos += 1; // Move by 1 to find overlapping matches
    }
  }

  if (matches.length === 0) {
    return escapeHtml(text);
  }

  // Merge overlapping matches
  matches.sort((a, b) => a.start - b.start);
  const merged: Array<{start: number, end: number}> = [];

  for (const match of matches) {
    if (merged.length === 0 || match.start > merged[merged.length - 1].end) {
      merged.push(match);
    } else {
      // Extend previous match
      merged[merged.length - 1].end = Math.max(merged[merged.length - 1].end, match.end);
    }
  }

  // Build highlighted HTML
  let result = '';
  let lastEnd = 0;

  for (const match of merged) {
    // Add text before match
    result += escapeHtml(text.substring(lastEnd, match.start));

    // Add highlighted match with custom class
    result += `<mark class="search-highlight">${escapeHtml(text.substring(match.start, match.end))}</mark>`;

    lastEnd = match.end;
  }

  // Add remaining text
  result += escapeHtml(text.substring(lastEnd));

  return result;
}

function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

/**
 * Main highlighting function - shows matched context with all occurrences highlighted
 */
export function highlightMatches(
  content: string,
  query: string,
  maxLength: number = 300
): string {
  if (!query || !content) {
    return escapeHtml(content.substring(0, maxLength)) + (content.length > maxLength ? '...' : '');
  }

  // Extract meaningful query words
  const queryWords = extractQueryWords(query);

  if (queryWords.length === 0) {
    return escapeHtml(content.substring(0, maxLength)) + (content.length > maxLength ? '...' : '');
  }

  // Find best matching snippet with context
  const snippet = findBestMatchingSnippet(content, queryWords, maxLength);

  // Highlight all matching words in the snippet
  return highlightAllMatches(snippet.text, queryWords);
}

/**
 * Extract a snippet with context around first match (legacy compatibility)
 */
export function extractSnippet(
  text: string,
  query: string,
  contextLength: number = 50
): string {
  if (!query || !text) return text;

  const queryWords = extractQueryWords(query);
  if (queryWords.length === 0) return text;

  const snippet = findBestMatchingSnippet(text, queryWords, contextLength * 2);
  return snippet.text;
}

function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
