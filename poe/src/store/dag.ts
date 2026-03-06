import { atom } from "jotai";
import type { DagNode, DagEdge, ProjectInfo } from "../types";

// ── Atoms ──────────────────────────────────────────────────────────────────────

export const projectAtom = atom<ProjectInfo | null>(null);

export const nodesAtom = atom<Map<string, DagNode>>(new Map());

export const edgesAtom = atom<Map<string, DagEdge>>(new Map());

// ── Derived ────────────────────────────────────────────────────────────────────

export const nodesListAtom = atom((get) => Array.from(get(nodesAtom).values()));

export const edgesListAtom = atom((get) => Array.from(get(edgesAtom).values()));
