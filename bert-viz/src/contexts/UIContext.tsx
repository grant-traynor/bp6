import { createContext, useContext } from "react";
import type { ViewType } from "../components/layout/Navigation";

export type { ViewType };

export interface UIContextValue {
  isDark: boolean;
  view: ViewType;
  panelWidth: number;
}

export const UIContext = createContext<UIContextValue | null>(null);

export interface UIProviderProps {
  value: UIContextValue;
  children: React.ReactNode;
}

export function UIProvider({ value, children }: UIProviderProps) {
  return (
    <UIContext.Provider value={value}>
      {children}
    </UIContext.Provider>
  );
}

export function useUIContext(): UIContextValue {
  const ctx = useContext(UIContext);
  if (ctx === null) {
    throw new Error("useUIContext must be used within a UIProvider");
  }
  return ctx;
}
