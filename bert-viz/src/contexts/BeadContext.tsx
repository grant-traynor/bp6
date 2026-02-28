import { createContext, useContext } from "react";
import type { BeadNode } from "../api";

export interface BeadContextValue {
  selectedBead: BeadNode | null;
  handleBeadClick: (bead: BeadNode) => void;
  sessionsByBead: Record<string, string[]>;
}

export const BeadContext = createContext<BeadContextValue | null>(null);

export interface BeadProviderProps {
  value: BeadContextValue;
  children: React.ReactNode;
}

export function BeadProvider({ value, children }: BeadProviderProps) {
  return (
    <BeadContext.Provider value={value}>
      {children}
    </BeadContext.Provider>
  );
}

export function useBeadContext(): BeadContextValue {
  const ctx = useContext(BeadContext);
  if (ctx === null) {
    throw new Error("useBeadContext must be used within a BeadProvider");
  }
  return ctx;
}
