import { useEffect, useCallback, useRef } from "react";

export interface KeyboardNavigationOptions {
  enabled?: boolean;
  onArrowUp?: () => void;
  onArrowDown?: () => void;
  onEnter?: () => void;
  onEscape?: () => void;
  onCtrlE?: () => void;
  onCtrlD?: () => void;
  preventDefault?: boolean;
}

/**
 * Custom hook for keyboard navigation
 * Handles arrow keys, Enter, Escape, and common shortcuts
 */
export function useKeyboardNavigation(options: KeyboardNavigationOptions) {
  const {
    enabled = true,
    onArrowUp,
    onArrowDown,
    onEnter,
    onEscape,
    onCtrlE,
    onCtrlD,
    preventDefault = true,
  } = options;

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!enabled) return;

      // Handle arrow keys
      if (e.key === "ArrowUp" && onArrowUp) {
        if (preventDefault) e.preventDefault();
        onArrowUp();
        return;
      }

      if (e.key === "ArrowDown" && onArrowDown) {
        if (preventDefault) e.preventDefault();
        onArrowDown();
        return;
      }

      // Handle Enter
      if (e.key === "Enter" && onEnter) {
        if (preventDefault) e.preventDefault();
        onEnter();
        return;
      }

      // Handle Escape
      if (e.key === "Escape" && onEscape) {
        if (preventDefault) e.preventDefault();
        onEscape();
        return;
      }

      // Handle Ctrl+E (Edit)
      if ((e.ctrlKey || e.metaKey) && e.key === "e" && onCtrlE) {
        e.preventDefault();
        onCtrlE();
        return;
      }

      // Handle Ctrl+D (Delete)
      if ((e.ctrlKey || e.metaKey) && e.key === "d" && onCtrlD) {
        e.preventDefault();
        onCtrlD();
        return;
      }
    },
    [enabled, onArrowUp, onArrowDown, onEnter, onEscape, onCtrlE, onCtrlD, preventDefault]
  );

  useEffect(() => {
    if (!enabled) return;

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [enabled, handleKeyDown]);
}

/**
 * Hook for managing list navigation state
 * Provides selected index and handlers for arrow key navigation
 */
export function useListNavigation<T>(items: T[], options?: {
  initialIndex?: number;
  loop?: boolean;
  onSelect?: (item: T, index: number) => void;
}) {
  const {
    initialIndex = -1,
    loop = true,
    onSelect,
  } = options || {};

  const [selectedIndex, setSelectedIndex] = React.useState(initialIndex);
  const itemsRef = useRef<T[]>(items);

  // Update ref when items change
  useEffect(() => {
    itemsRef.current = items;
    // Reset selection if it's out of bounds
    if (selectedIndex >= items.length) {
      setSelectedIndex(items.length > 0 ? 0 : -1);
    }
  }, [items, selectedIndex]);

  const moveUp = useCallback(() => {
    setSelectedIndex((prev) => {
      if (items.length === 0) return -1;
      if (prev <= 0) {
        return loop ? items.length - 1 : 0;
      }
      return prev - 1;
    });
  }, [items.length, loop]);

  const moveDown = useCallback(() => {
    setSelectedIndex((prev) => {
      if (items.length === 0) return -1;
      if (prev >= items.length - 1) {
        return loop ? 0 : items.length - 1;
      }
      return prev + 1;
    });
  }, [items.length, loop]);

  const selectCurrent = useCallback(() => {
    if (selectedIndex >= 0 && selectedIndex < items.length) {
      const item = items[selectedIndex];
      onSelect?.(item, selectedIndex);
    }
  }, [items, selectedIndex, onSelect]);

  const reset = useCallback(() => {
    setSelectedIndex(initialIndex);
  }, [initialIndex]);

  return {
    selectedIndex,
    setSelectedIndex,
    moveUp,
    moveDown,
    selectCurrent,
    reset,
    hasSelection: selectedIndex >= 0 && selectedIndex < items.length,
    selectedItem: selectedIndex >= 0 && selectedIndex < items.length ? items[selectedIndex] : null,
  };
}

// Need to import React for useState
import React from "react";
