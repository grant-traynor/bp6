import { createContext, useContext } from "react";
import type { ProjectViewModel } from "../api";

export interface ProjectContextValue {
  viewModel: ProjectViewModel | null;
  currentProjectPath: string;
  hasProject: boolean;
  loading: boolean;
  processingData: boolean;
  loadData: () => void;
}

export const ProjectContext = createContext<ProjectContextValue | null>(null);

export interface ProjectProviderProps {
  value: ProjectContextValue;
  children: React.ReactNode;
}

export function ProjectProvider({ value, children }: ProjectProviderProps) {
  return (
    <ProjectContext.Provider value={value}>
      {children}
    </ProjectContext.Provider>
  );
}

export function useProjectContext(): ProjectContextValue {
  const ctx = useContext(ProjectContext);
  if (ctx === null) {
    throw new Error("useProjectContext must be used within a ProjectProvider");
  }
  return ctx;
}
