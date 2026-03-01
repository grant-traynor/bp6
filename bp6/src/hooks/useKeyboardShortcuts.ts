import { useEffect } from "react";
import type { BeadNode } from "../api";

export interface UseKeyboardShortcutsInput {
  expandAll: () => void;
  collapseAll: (viewModelTree?: BeadNode[]) => void;
  loadData: () => void;
  handleOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => Promise<void>;
  setFilterText: (value: string) => void;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  setSelectedBead: React.Dispatch<React.SetStateAction<BeadNode | null>>;
  setIsCreating: React.Dispatch<React.SetStateAction<boolean>>;
  setBeadDetailOpen: React.Dispatch<React.SetStateAction<boolean>>;
  handleStartCreate: () => void;
}

/**
 * Registers a global keydown listener for the following shortcuts:
 *   /         - Focus search input
 *   Escape    - Clear selection / close bead detail
 *   + / =     - Expand all nodes
 *   - / _     - Collapse all nodes
 *   n         - Start creating a new bead
 *   r         - Refresh data
 *   c         - Open product-manager chat
 *
 * Shortcuts are suppressed when focus is on an input/textarea, and when any
 * modifier key (Cmd/Ctrl/Alt) is held.
 */
export function useKeyboardShortcuts(input: UseKeyboardShortcutsInput): void {
  const {
    expandAll,
    collapseAll,
    loadData,
    handleOpenChat,
    searchInputRef,
    setSelectedBead,
    setIsCreating,
    setBeadDetailOpen,
    handleStartCreate,
  } = input;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        if (e.key === "Escape") {
          e.preventDefault();
          (e.target as HTMLElement).blur();
        }
        return;
      }

      // Don't intercept keyboard shortcuts with modifiers (Cmd/Ctrl/Alt)
      if (e.metaKey || e.ctrlKey || e.altKey) {
        return;
      }

      switch (e.key) {
        case "/":
          e.preventDefault();
          searchInputRef.current?.focus();
          break;
        case "Escape":
          setSelectedBead(null);
          setIsCreating(false);
          setBeadDetailOpen(false);
          break;
        case "+":
        case "=":
          e.preventDefault();
          expandAll();
          break;
        case "-":
        case "_":
          e.preventDefault();
          collapseAll();
          break;
        case "n":
          e.preventDefault();
          handleStartCreate();
          break;
        case "r":
          e.preventDefault();
          loadData();
          break;
        case "c":
          e.preventDefault();
          handleOpenChat("product-manager").catch((err) =>
            console.error("Chat open failed:", err)
          );
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    expandAll,
    collapseAll,
    loadData,
    handleOpenChat,
    searchInputRef,
    setSelectedBead,
    setIsCreating,
    setBeadDetailOpen,
    handleStartCreate,
  ]);
}
