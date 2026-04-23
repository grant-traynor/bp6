import { useState, useEffect, useCallback } from "react";
import {
  updateBead,
  createBead,
  closeBead,
  reopenBead,
  claimBead,
  beadNodeToBead,
  type BeadNode,
  type ProjectViewModel,
} from "../api";

export interface UseBeadEditingReturn {
  selectedBead: BeadNode | null;
  setSelectedBead: React.Dispatch<React.SetStateAction<BeadNode | null>>;
  isCreating: boolean;
  setIsCreating: React.Dispatch<React.SetStateAction<boolean>>;
  editForm: Partial<BeadNode>;
  setEditForm: React.Dispatch<React.SetStateAction<Partial<BeadNode>>>;
  beadDetailOpen: boolean;
  setBeadDetailOpen: React.Dispatch<React.SetStateAction<boolean>>;
  handleBeadClick: (bead: BeadNode) => void;
  handleStartCreate: (explicitParentId?: string) => void;
  handleSaveCreate: (viewModel: ProjectViewModel | null) => Promise<void>;
  handleSaveEdit: (viewModel: ProjectViewModel | null) => Promise<void>;
  handleCloseBead: (beadId: string) => Promise<void>;
  handleReopenBead: (beadId: string) => Promise<void>;
  handleClaimBead: (beadId: string) => Promise<void>;
  toggleFavorite: (bead: BeadNode) => Promise<void>;
}

function flattenTree(nodes: BeadNode[]): BeadNode[] {
  const result: BeadNode[] = [];
  const traverse = (nodeList: BeadNode[]) => {
    nodeList.forEach((node) => {
      result.push(node);
      if (node.children.length > 0) {
        traverse(node.children);
      }
    });
  };
  traverse(nodes);
  return result;
}

export function useBeadEditing(
  loadData: () => void,
  beads: BeadNode[]
): UseBeadEditingReturn {
  const [selectedBead, setSelectedBead] = useState<BeadNode | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [editForm, setEditForm] = useState<Partial<BeadNode>>({});
  const [beadDetailOpen, setBeadDetailOpen] = useState(false);

  // Keep selectedBead synchronized with latest bead data from view model
  useEffect(() => {
    if (selectedBead && beads.length > 0) {
      const updatedBead = beads.find((b) => b.id === selectedBead.id);
      if (updatedBead) {
        setSelectedBead(updatedBead);
      }
    }
  }, [beads]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleBeadClick = useCallback((bead: BeadNode) => {
    setSelectedBead(bead);
    setIsCreating(false);
  }, []);

  const handleStartCreate = useCallback((explicitParentId?: string) => {
    // Use explicitly-provided parent, or fall back to the currently-selected bead.
    const parentId = explicitParentId ?? selectedBead?.id;
    setSelectedBead(null);
    setEditForm({
      id: `bp6-${Math.random().toString(36).slice(2, 5)}`,
      title: "",
      status: "open",
      priority: 2,
      issueType: "task",
      dependencies: [],
      createdAt: new Date().toISOString(),
      acceptanceCriteria: [],
      ...(parentId ? { parent: parentId } : {}),
    } as Partial<BeadNode>);
    setIsCreating(true);
    setBeadDetailOpen(true);
  }, [selectedBead]);

  const handleSaveEdit = useCallback(
    async (viewModel: ProjectViewModel | null) => {
      if (selectedBead && editForm) {
        try {
          const allBeads = flattenTree(viewModel?.tree || []);
          const currentParent = allBeads.find((b) =>
            b.dependencies?.some(
              (d) => d.issue_id === selectedBead.id && d.type === "parent-child"
            )
          );
          const updatedNode = { ...editForm };
          if (updatedNode.parent !== currentParent?.id) {
            updatedNode.dependencies = (updatedNode.dependencies || []).filter(
              (d) => d.type !== "parent-child"
            );
            if (updatedNode.parent) {
              updatedNode.dependencies.push({
                issue_id: updatedNode.id!,
                depends_on_id: updatedNode.parent,
                type: "parent-child",
              });
            }
          }
          const updated = beadNodeToBead(updatedNode);
          await updateBead(updated as Parameters<typeof updateBead>[0]);
          setSelectedBead(updatedNode as BeadNode);
          loadData();
        } catch (error) {
          alert(`Failed to save bead: ${error}`);
        }
      }
    },
    [selectedBead, editForm, loadData]
  );

  const handleSaveCreate = useCallback(
    async (viewModel: ProjectViewModel | null) => {
      console.log("handleSaveCreate called", { editForm });
      if (editForm.title) {
        console.log("Title exists, proceeding with create");
        try {
          const newNode = { ...editForm };
          console.log("newNode prepared:", newNode);
          if (newNode.parent) {
            newNode.dependencies = [
              ...(newNode.dependencies || []),
              {
                issue_id: newNode.id!,
                depends_on_id: newNode.parent,
                type: "parent-child",
              },
            ];
          }
          const newBead = beadNodeToBead(newNode);
          console.log("Calling createBead...");
          const createdId = await createBead(newBead as Parameters<typeof createBead>[0]);
          console.log("Created bead ID:", createdId);
          setIsCreating(false);
          loadData();
          const allBeads = flattenTree(viewModel?.tree || []);
          const createdBead = allBeads.find((b: BeadNode) => b.id === createdId);
          if (createdBead) {
            setSelectedBead(createdBead);
          }
        } catch (error) {
          console.error("Error creating bead:", error);
          alert(`Failed to create bead: ${error}`);
        }
      } else {
        console.log("No title, skipping create");
      }
    },
    [editForm, loadData]
  );

  const handleCloseBead = useCallback(
    async (beadId: string) => {
      try {
        await closeBead(beadId);
        setSelectedBead(null);
        loadData();
      } catch (error) {
        console.error("Failed to close bead:", error);
      }
    },
    [loadData]
  );

  const handleReopenBead = useCallback(
    async (beadId: string) => {
      try {
        await reopenBead(beadId);
        loadData();
      } catch (error) {
        console.error("Failed to reopen bead:", error);
      }
    },
    [loadData]
  );

  const handleClaimBead = useCallback(
    async (beadId: string) => {
      try {
        await claimBead(beadId);
        loadData();
      } catch (error) {
        console.error("Failed to claim bead:", error);
      }
    },
    [loadData]
  );

  const toggleFavorite = useCallback(
    async (bead: BeadNode) => {
      try {
        const updated = beadNodeToBead({
          ...bead,
          isFavorite: !bead.isFavorite,
        });
        await updateBead(updated as Parameters<typeof updateBead>[0]);
        loadData();
      } catch (error) {
        alert(`Failed to toggle favorite: ${error}`);
      }
    },
    [loadData]
  );

  return {
    selectedBead,
    setSelectedBead,
    isCreating,
    setIsCreating,
    editForm,
    setEditForm,
    beadDetailOpen,
    setBeadDetailOpen,
    handleBeadClick,
    handleStartCreate,
    handleSaveCreate,
    handleSaveEdit,
    handleCloseBead,
    handleReopenBead,
    handleClaimBead,
    toggleFavorite,
  };
}
