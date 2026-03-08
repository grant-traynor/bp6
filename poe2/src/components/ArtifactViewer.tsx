import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Artifact } from '../types';

interface Props {
  artifact: Artifact;
  projectId: string;
  onClose: () => void;
}

export default function ArtifactViewer({ artifact, projectId, onClose }: Props) {
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>('read_artifact_content', { artifactId: artifact.id, projectId })
      .then(setContent)
      .catch((e) => setError(String(e)));
  }, [artifact.id, projectId]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
      <div className="flex flex-col bg-neutral-950 border-2 border-neutral-700 rounded w-[800px] h-[600px] shadow-2xl">
        <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-800 shrink-0">
          <div className="min-w-0">
            <span className="text-xs font-semibold text-neutral-200">{artifact.filename}</span>
            <span className="ml-2 text-[10px] text-neutral-500 font-mono">{artifact.artifactType}</span>
          </div>
          <button
            className="text-neutral-500 hover:text-neutral-200 text-xs px-2 py-1"
            onClick={onClose}
          >
            ✕ close
          </button>
        </div>
        <div className="flex-1 overflow-y-auto p-4">
          {error ? (
            <p className="text-red-400 text-sm">{error}</p>
          ) : content === null ? (
            <p className="text-neutral-600 text-sm">Loading…</p>
          ) : (
            <pre className="text-neutral-300 text-[12px] font-mono whitespace-pre-wrap leading-5">
              {content}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}
