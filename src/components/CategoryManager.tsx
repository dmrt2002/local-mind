import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import "./CategoryManager.css";

interface CategoryManagerProps {
  onCategoryCreated?: () => void;
}

export default function CategoryManager({ onCategoryCreated }: CategoryManagerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [name, setName] = useState("");
  const [emoji, setEmoji] = useState("📁");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!name.trim()) {
      setError("Category name is required");
      return;
    }

    setCreating(true);
    setError(null);

    try {
      await invoke("create_category", {
        request: {
          name: name.trim(),
          parent_id: null,
          emoji: emoji || "📁",
        },
      });

      setSuccess(true);
      setName("");
      setEmoji("📁");

      setTimeout(() => {
        setSuccess(false);
        setIsOpen(false);
      }, 1500);

      if (onCategoryCreated) {
        onCategoryCreated();
      }
    } catch (err) {
      console.error("Failed to create category:", err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreating(false);
    }
  };

  if (!isOpen) {
    return (
      <button className="add-category-button" onClick={() => setIsOpen(true)}>
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M8 3V13M3 8H13"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        New Category
      </button>
    );
  }

  return (
    <div className="category-manager-modal">
      <div className="category-manager-overlay" onClick={() => setIsOpen(false)} />
      <div className="category-manager-content">
        <div className="manager-header">
          <h3>Create New Category</h3>
          <button className="close-button" onClick={() => setIsOpen(false)}>
            <svg
              width="20"
              height="20"
              viewBox="0 0 16 16"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M12 4L4 12M4 4L12 12"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
        </div>

        <form onSubmit={handleCreate} className="category-form">
          <div className="form-group">
            <label htmlFor="emoji">Icon</label>
            <input
              id="emoji"
              type="text"
              value={emoji}
              onChange={(e) => setEmoji(e.target.value.slice(0, 2))}
              className="emoji-input"
              placeholder="📁"
              maxLength={2}
            />
            <small>Choose an emoji to represent this category</small>
          </div>

          <div className="form-group">
            <label htmlFor="name">Category Name *</label>
            <input
              id="name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="name-input"
              placeholder="e.g., Code Snippets, Documentation, Ideas..."
              autoFocus
              disabled={creating}
            />
          </div>

          {error && (
            <div className="error-message">
              {error}
            </div>
          )}

          {success && (
            <div className="success-message">
              ✅ Category created successfully!
            </div>
          )}

          <div className="form-actions">
            <button
              type="button"
              onClick={() => setIsOpen(false)}
              className="cancel-button"
              disabled={creating}
            >
              Cancel
            </button>
            <button
              type="submit"
              className="create-button"
              disabled={creating || !name.trim()}
            >
              {creating ? "Creating..." : "Create Category"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
