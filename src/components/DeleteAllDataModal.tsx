import React, { useState } from 'react';
import './DeleteAllDataModal.css';

interface DeleteAllDataModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  storageStats: {
    snippets_count: number;
    categories_count: number;
    embeddings_count: number;
  };
}

const DeleteAllDataModal: React.FC<DeleteAllDataModalProps> = ({
  isOpen,
  onClose,
  onConfirm,
  storageStats,
}) => {
  const [confirmText, setConfirmText] = useState('');
  const [isDeleting, setIsDeleting] = useState(false);

  if (!isOpen) return null;

  const isConfirmValid = confirmText === 'DELETE';

  const handleConfirm = async () => {
    if (!isConfirmValid || isDeleting) return;

    setIsDeleting(true);
    try {
      await onConfirm();
      onClose();
    } finally {
      setIsDeleting(false);
      setConfirmText('');
    }
  };

  const handleClose = () => {
    if (isDeleting) return;
    setConfirmText('');
    onClose();
  };

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-content delete-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>⚠️ Delete All Data</h2>
          <button className="close-button" onClick={handleClose} disabled={isDeleting}>
            ×
          </button>
        </div>

        <div className="modal-body">
          <div className="warning-message">
            <p className="warning-text">
              <strong>This action cannot be undone!</strong>
            </p>
            <p>
              You are about to permanently delete:
            </p>
          </div>

          <div className="data-summary">
            <div className="data-item">
              <span className="data-icon">📝</span>
              <span className="data-count">{storageStats.snippets_count.toLocaleString()}</span>
              <span className="data-label">Text snippets & commands</span>
            </div>
            <div className="data-item">
              <span className="data-icon">📷</span>
              <span className="data-count">All</span>
              <span className="data-label">Screenshots & files</span>
            </div>
            <div className="data-item">
              <span className="data-icon">🏷️</span>
              <span className="data-count">{storageStats.categories_count.toLocaleString()}</span>
              <span className="data-label">Categories</span>
            </div>
            <div className="data-item">
              <span className="data-icon">🧠</span>
              <span className="data-count">{storageStats.embeddings_count.toLocaleString()}</span>
              <span className="data-label">AI embeddings</span>
            </div>
          </div>

          <div className="confirmation-section">
            <label htmlFor="confirm-input">
              Type <strong>DELETE</strong> to confirm:
            </label>
            <input
              id="confirm-input"
              type="text"
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              placeholder="Type DELETE here"
              className="confirm-input"
              disabled={isDeleting}
              autoFocus
            />
          </div>
        </div>

        <div className="modal-footer">
          <button
            className="cancel-button"
            onClick={handleClose}
            disabled={isDeleting}
          >
            Cancel
          </button>
          <button
            className="delete-button"
            onClick={handleConfirm}
            disabled={!isConfirmValid || isDeleting}
          >
            {isDeleting ? 'Deleting...' : 'Delete All Data'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default DeleteAllDataModal;
