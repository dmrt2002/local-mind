import { useState, useEffect, useCallback, memo } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import CategoryManager from "./CategoryManager";
import ConfirmDialog from "./ConfirmDialog";
import "./CategoryTree.css";

export interface Category {
  id: number;
  name: string;
  parent_id: number | null;
  emoji: string;
  created_at: string;
  snippet_count?: number;
}

export interface Snippet {
  id: number;
  content: string;
  summary: string | null;
  created_at: string;
  source_app: string | null;
  metadata: string | null;
}

interface CategoryTreeProps {
  selectedCategoryId: number | null;
  onSelectCategory: (categoryId: number | null) => void;
  onSelectSnippet: (snippetId: number) => void;
}

interface TreeNodeProps {
  category: Category;
  level: number;
  isSelected: boolean;
  selectedCategoryId: number | null;
  onSelect: (id: number) => void;
  onSelectSnippet: (snippetId: number) => void;
  draggedSnippetId: number | null;
  onStartDrag: (snippetId: number) => void;
  onEndDrag: () => void;
  onDropOnCategory: (categoryId: number) => void;
  onRefreshTree: () => void;
}

const TreeNode = memo(({ category, level, isSelected, selectedCategoryId, onSelect, onSelectSnippet, draggedSnippetId, onStartDrag, onEndDrag, onDropOnCategory, onRefreshTree }: TreeNodeProps) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [children, setChildren] = useState<Category[]>([]);
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [loading, setLoading] = useState(false);
  const [hasLoadedChildren, setHasLoadedChildren] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [editName, setEditName] = useState(category.name);
  const [editEmoji, setEditEmoji] = useState(category.emoji);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  // Auto-expand when this category is selected
  useEffect(() => {
    if (isSelected && !isExpanded && !hasLoadedChildren) {
      // Load and expand the category
      setLoading(true);
      Promise.all([
        invoke<Category[]>("get_child_categories", {
          parentId: category.id,
          includeCounts: true,
        }),
        invoke<Snippet[]>("get_snippets_by_category", {
          categoryId: category.id,
          limit: 100,
          offset: 0,
        })
      ])
        .then(([childCategories, categorySnippets]) => {
          setChildren(childCategories);
          setSnippets(categorySnippets);
          setHasLoadedChildren(true);
          setIsExpanded(true);
        })
        .catch((err) => {
          console.error("Failed to load child categories and snippets:", err);
        })
        .finally(() => {
          setLoading(false);
        });
    } else if (isSelected && !isExpanded && hasLoadedChildren) {
      // Already loaded, just expand
      setIsExpanded(true);
    }
  }, [isSelected, category.id, isExpanded, hasLoadedChildren]);

  const toggleExpand = useCallback(async () => {
    if (!isExpanded && !hasLoadedChildren) {
      // Load children and snippets when expanding for the first time
      setLoading(true);
      try {
        // Load both in parallel for faster performance
        const [childCategories, categorySnippets] = await Promise.all([
          invoke<Category[]>("get_child_categories", {
            parentId: category.id,
            includeCounts: true,
          }),
          invoke<Snippet[]>("get_snippets_by_category", {
            categoryId: category.id,
            limit: 100,
            offset: 0,
          })
        ]);

        setChildren(childCategories);
        setSnippets(categorySnippets);
        setHasLoadedChildren(true);
      } catch (err) {
        console.error("Failed to load child categories and snippets:", err);
      } finally {
        setLoading(false);
      }
    }
    setIsExpanded(!isExpanded);
  }, [isExpanded, hasLoadedChildren, category.id]);

  const handleClick = useCallback(() => {
    onSelect(category.id);
  }, [onSelect, category.id]);

  // Mouse-based Drag and Drop (replaces broken HTML5 DnD)
  const handleMouseUp = useCallback(async () => {
    if (draggedSnippetId !== null) {
      console.log("🎯 Mouse released over category:", category.name, "snippetId:", draggedSnippetId);

      try {
        await invoke("assign_snippet_to_category", {
          request: {
            snippet_id: draggedSnippetId,
            category_id: category.id,
            is_manual: true,
          }
        });

        console.log("✅ Successfully moved snippet", draggedSnippetId, "to category", category.id);

        // Refresh the entire tree to update all categories
        onRefreshTree();

        onEndDrag();
      } catch (err) {
        console.error("❌ Failed to move snippet:", err);
        onEndDrag();
      }
    }
  }, [draggedSnippetId, category.id, category.name, onRefreshTree, onEndDrag]);

  const handleMouseEnter = useCallback(() => {
    if (draggedSnippetId !== null) {
      setIsDragOver(true);
      console.log("⬇️ Mouse entered category:", category.name);
    }
  }, [draggedSnippetId, category.name]);

  const handleMouseLeave = useCallback(() => {
    if (draggedSnippetId !== null) {
      setIsDragOver(false);
      console.log("⬆️ Mouse left category:", category.name);
    }
  }, [draggedSnippetId, category.name]);

  // Category Edit Handlers
  const handleDoubleClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setIsEditing(true);
    setEditName(category.name);
    setEditEmoji(category.emoji);
  }, [category.name, category.emoji]);

  const handleSaveEdit = useCallback(async () => {
    const trimmedName = editName.trim();
    if (!trimmedName) {
      setEditName(category.name);
      setIsEditing(false);
      return;
    }

    try {
      console.log("Updating category:", {
        id: category.id,
        name: trimmedName,
        emoji: editEmoji,
      });

      await invoke("update_category", {
        request: {
          id: category.id,
          name: trimmedName,
          emoji: editEmoji,
        }
      });

      console.log("Category updated successfully");
      setIsEditing(false);
      // Update local state
      category.name = trimmedName;
      category.emoji = editEmoji;
    } catch (err) {
      console.error("Failed to update category:", err);
      setEditName(category.name);
      setEditEmoji(category.emoji);
      setIsEditing(false);
    }
  }, [category, editName, editEmoji]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSaveEdit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      setIsEditing(false);
      setEditName(category.name);
      setEditEmoji(category.emoji);
    }
  }, [handleSaveEdit, category.name, category.emoji]);

  const handleBlur = useCallback(() => {
    handleSaveEdit();
  }, [handleSaveEdit]);

  const handleDeleteClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setShowDeleteConfirm(true);
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    setShowDeleteConfirm(false);

    try {
      await invoke("delete_category", { id: category.id });
      console.log("✅ Category deleted successfully");
      // Refresh the tree
      onRefreshTree();
    } catch (err) {
      console.error("Failed to delete category:", err);
      alert(`Failed to delete category: ${err}`);
    }
  }, [category.id, onRefreshTree]);

  const handleCancelDelete = useCallback(() => {
    setShowDeleteConfirm(false);
  }, []);

  const hasChildren = children.length > 0 || snippets.length > 0 || !hasLoadedChildren;
  const paddingLeft = level * 16 + 8;

  return (
    <div className="tree-node">
      <div
        className={`tree-node-content ${isSelected ? "selected" : ""} ${isDragOver ? "drag-over" : ""}`}
        style={{ paddingLeft: `${paddingLeft}px` }}
        onMouseUp={handleMouseUp}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {hasChildren && (
          <button
            className="expand-button"
            onClick={toggleExpand}
            disabled={loading}
          >
            {loading ? (
              <div className="mini-spinner" />
            ) : (
              <svg
                width="12"
                height="12"
                viewBox="0 0 12 12"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
                style={{
                  transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)",
                  transition: "transform 0.2s ease",
                }}
              >
                <path
                  d="M4.5 2.5L7.5 6L4.5 9.5"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            )}
          </button>
        )}
        {isEditing ? (
          <div className="category-edit-mode">
            <input
              type="text"
              value={editEmoji}
              onChange={(e) => setEditEmoji(e.target.value)}
              className="edit-emoji-input"
              placeholder="📁"
              maxLength={2}
              onBlur={handleBlur}
              onKeyDown={handleKeyDown}
            />
            <input
              type="text"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              className="edit-name-input"
              placeholder="Category name"
              onBlur={handleBlur}
              onKeyDown={handleKeyDown}
              autoFocus
            />
          </div>
        ) : (
          <div className="category-button-container">
            <div
              className="category-button"
              onClick={handleClick}
              onDoubleClick={handleDoubleClick}
              role="button"
              tabIndex={0}
            >
              <span className="category-emoji">{category.emoji}</span>
              <span className="category-name">{category.name}</span>
              {category.snippet_count !== undefined && category.snippet_count > 0 && (
                <span className="category-count">{category.snippet_count}</span>
              )}
            </div>
            <button
              className="category-delete-button"
              onClick={handleDeleteClick}
              title="Delete category"
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 16 16"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M2 4H14"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M12.6667 4V13.3333C12.6667 13.7 12.5 14 12.3333 14H3.66667C3.5 14 3.33333 13.7 3.33333 13.3333V4"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M5.33331 4V2.66667C5.33331 2.3 5.49998 2 5.66665 2H10.3333C10.5 2 10.6666 2.3 10.6666 2.66667V4"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </button>
          </div>
        )}
      </div>
      {isExpanded && (children.length > 0 || snippets.length > 0) && (
        <div className="tree-node-children">
          {/* Render child categories first */}
          {children.map((child) => (
            <TreeNode
              key={child.id}
              category={child}
              level={level + 1}
              isSelected={child.id === selectedCategoryId}
              selectedCategoryId={selectedCategoryId}
              onSelect={onSelect}
              onSelectSnippet={onSelectSnippet}
              draggedSnippetId={draggedSnippetId}
              onStartDrag={onStartDrag}
              onEndDrag={onEndDrag}
              onDropOnCategory={onDropOnCategory}
              onRefreshTree={onRefreshTree}
            />
          ))}
          {/* Render snippets */}
          {snippets.map((snippet) => (
            <div
              key={`snippet-${snippet.id}`}
              className={`tree-node-content snippet-item ${draggedSnippetId === snippet.id ? "dragging" : ""}`}
              style={{ paddingLeft: `${paddingLeft + 16}px` }}
              onMouseDown={(e) => {
                e.preventDefault();
                console.log("✅ Mouse down on snippet:", snippet.id);
                onStartDrag(snippet.id);
              }}
            >
              <button
                className="category-button snippet-button"
                onClick={() => onSelectSnippet(snippet.id)}
                title={snippet.content.substring(0, 200)}
              >
                <span className="category-emoji">📄</span>
                <span className="category-name snippet-name">
                  {snippet.summary || snippet.content.substring(0, 50) + "..."}
                </span>
              </button>
            </div>
          ))}
        </div>
      )}
      {showDeleteConfirm && (
        <ConfirmDialog
          title="Delete Category"
          message={`Are you sure you want to delete "${category.name}"? This will also delete all subcategories and their snippets will be uncategorized.`}
          confirmText="Delete"
          cancelText="Cancel"
          onConfirm={handleConfirmDelete}
          onCancel={handleCancelDelete}
          variant="danger"
        />
      )}
    </div>
  );
});

export default function CategoryTree({
  selectedCategoryId,
  onSelectCategory,
  onSelectSnippet,
}: CategoryTreeProps) {
  const [rootCategories, setRootCategories] = useState<Category[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draggedSnippetId, setDraggedSnippetId] = useState<number | null>(null);

  useEffect(() => {
    loadRootCategories();
  }, []);

  // Global mouseup to clear dragging state
  useEffect(() => {
    const handleGlobalMouseUp = () => {
      if (draggedSnippetId !== null) {
        console.log("🏁 Mouse released (global) - clearing drag state");
        setDraggedSnippetId(null);
      }
    };

    window.addEventListener("mouseup", handleGlobalMouseUp);
    return () => window.removeEventListener("mouseup", handleGlobalMouseUp);
  }, [draggedSnippetId]);

  const loadRootCategories = async () => {
    setLoading(true);
    setError(null);

    try {
      const categories = await invoke<Category[]>("get_root_categories", {
        includeCounts: true,
      });
      setRootCategories(categories);
    } catch (err) {
      console.error("Failed to load root categories:", err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="category-tree">
        <div className="tree-header">
          <h3>Categories</h3>
        </div>
        <div className="tree-loading">
          <div className="spinner"></div>
          Loading categories...
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="category-tree">
        <div className="tree-header">
          <h3>Categories</h3>
        </div>
        <div className="tree-error">
          <p>Error: {error}</p>
          <button onClick={loadRootCategories} className="retry-button">
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (rootCategories.length === 0) {
    return (
      <div className="category-tree">
        <div className="tree-header">
          <h3>Categories</h3>
        </div>
        <div className="tree-empty">
          <p>No categories yet</p>
          <small>Categories will appear here once created</small>
        </div>
      </div>
    );
  }

  return (
    <div className="category-tree">
      <div className="tree-header">
        <h3>Categories</h3>
      </div>
      <div className="tree-content">
        {rootCategories.map((category) => (
          <TreeNode
            key={category.id}
            category={category}
            level={0}
            isSelected={category.id === selectedCategoryId}
            selectedCategoryId={selectedCategoryId}
            onSelect={onSelectCategory}
            onSelectSnippet={onSelectSnippet}
            draggedSnippetId={draggedSnippetId}
            onStartDrag={setDraggedSnippetId}
            onEndDrag={() => setDraggedSnippetId(null)}
            onDropOnCategory={() => {}}
            onRefreshTree={loadRootCategories}
          />
        ))}
      </div>
      <div className="tree-footer">
        <CategoryManager onCategoryCreated={loadRootCategories} />
      </div>
    </div>
  );
}
