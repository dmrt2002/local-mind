import { useCallback, useState } from "react";
import CategoryTree from "./CategoryTree";
import ContentPanel from "./ContentPanel";
import SuggestionsPanel from "./SuggestionsPanel";
import ContentTypeTabs, { ContentType } from "./ContentTypeTabs";
import "./HomeView.css";

interface HomeViewProps {
  selectedSnippetId: number | null;
  selectedCategoryId: number | null;
  onSelectSnippet: (snippetId: number | null) => void;
  onSelectCategory: (categoryId: number | null) => void;
}

export default function HomeView({
  selectedSnippetId,
  selectedCategoryId,
  onSelectSnippet,
  onSelectCategory,
}: HomeViewProps) {
  const [contentType, setContentType] = useState<ContentType>("all");

  const handleSelectCategory = useCallback((categoryId: number | null) => {
    onSelectCategory(categoryId);
    onSelectSnippet(null); // Clear snippet selection when category changes
  }, [onSelectCategory, onSelectSnippet]);

  const handleSelectSnippet = useCallback((snippetId: number) => {
    onSelectSnippet(snippetId);
  }, [onSelectSnippet]);

  const handleClearSnippet = useCallback(() => {
    onSelectSnippet(null);
  }, [onSelectSnippet]);

  const handleContentTypeChange = useCallback((type: ContentType) => {
    setContentType(type);
    // Keep category selected, only clear snippet selection
    // This allows the category to stay expanded and reload with new filter
    onSelectSnippet(null);
  }, [onSelectSnippet]);

  return (
    <div className="home-view">
      <ContentTypeTabs
        activeType={contentType}
        onTypeChange={handleContentTypeChange}
      />
      <div className="home-layout">
        <aside className="category-sidebar">
          <CategoryTree
            selectedCategoryId={selectedCategoryId}
            onSelectCategory={handleSelectCategory}
            onSelectSnippet={handleSelectSnippet}
            contentType={contentType}
          />
        </aside>
        <main className="content-main">
          <SuggestionsPanel />
          <ContentPanel
            categoryId={selectedCategoryId}
            snippetId={selectedSnippetId}
            contentType={contentType}
            onClearSnippet={handleClearSnippet}
          />
        </main>
      </div>
    </div>
  );
}
