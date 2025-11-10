import { useCallback } from "react";
import CategoryTree from "./CategoryTree";
import ContentPanel from "./ContentPanel";
import SuggestionsPanel from "./SuggestionsPanel";
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

  return (
    <div className="home-view">
      <div className="home-layout">
        <aside className="category-sidebar">
          <CategoryTree
            selectedCategoryId={selectedCategoryId}
            onSelectCategory={handleSelectCategory}
            onSelectSnippet={handleSelectSnippet}
          />
        </aside>
        <main className="content-main">
          <SuggestionsPanel />
          <ContentPanel
            categoryId={selectedCategoryId}
            snippetId={selectedSnippetId}
            onClearSnippet={handleClearSnippet}
          />
        </main>
      </div>
    </div>
  );
}
