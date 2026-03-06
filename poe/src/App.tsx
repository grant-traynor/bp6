import { useAtomValue } from "jotai";
import { projectAtom } from "./store/dag";
import { useDagEvents } from "./hooks/useDagEvents";
import { EmptyState } from "./components/EmptyState";
import { ProjectHeader } from "./components/ProjectHeader";
import { QueueView } from "./components/QueueView";

export default function App() {
  const project = useAtomValue(projectAtom);

  // Wire up all reactive event subscriptions
  useDagEvents();

  if (!project) {
    return <EmptyState />;
  }

  return (
    <div className="flex flex-col h-full">
      <ProjectHeader />
      <QueueView />
    </div>
  );
}
