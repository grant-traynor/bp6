import { diffLines } from 'diff';

interface DiffViewProps {
  filePath: string;
  oldString: string;
  newString: string;
  contextLines?: number;
}

type DiffLine =
  | { kind: 'add'; text: string }
  | { kind: 'del'; text: string }
  | { kind: 'ctx'; text: string }
  | { kind: 'skip'; count: number };

function buildDiffLines(oldStr: string, newStr: string, context: number): DiffLine[] {
  const changes = diffLines(oldStr, newStr);

  // Flatten to per-line entries
  type RawLine = { kind: 'add' | 'del' | 'ctx'; text: string };
  const raw: RawLine[] = [];

  for (const change of changes) {
    const lines = change.value.split('\n');
    if (lines[lines.length - 1] === '') lines.pop(); // trim trailing newline artifact
    const kind = change.added ? 'add' : change.removed ? 'del' : 'ctx';
    for (const text of lines) raw.push({ kind, text });
  }

  // Mark lines visible if within `context` lines of a change
  const changed = raw.map(l => l.kind !== 'ctx');
  const visible = new Array(raw.length).fill(false);
  for (let i = 0; i < raw.length; i++) {
    if (changed[i]) {
      for (let j = Math.max(0, i - context); j <= Math.min(raw.length - 1, i + context); j++) {
        visible[j] = true;
      }
    }
  }

  // Build output with skip markers for collapsed sections
  const result: DiffLine[] = [];
  let skipping = 0;
  for (let i = 0; i < raw.length; i++) {
    if (!visible[i]) { skipping++; continue; }
    if (skipping > 0) { result.push({ kind: 'skip', count: skipping }); skipping = 0; }
    result.push(raw[i]);
  }
  if (skipping > 0) result.push({ kind: 'skip', count: skipping });

  return result;
}

export function DiffView({ filePath, oldString, newString, contextLines = 3 }: DiffViewProps) {
  const isNewFile = !oldString;

  const lines: DiffLine[] = isNewFile
    ? newString.split('\n').map(text => ({ kind: 'add', text }))
    : buildDiffLines(oldString, newString, contextLines);

  const added = lines.filter(l => l.kind === 'add').length;
  const removed = lines.filter(l => l.kind === 'del').length;
  const summary = isNewFile
    ? `New file · ${added} lines`
    : removed === 0 ? `Added ${added} lines`
    : added === 0   ? `Removed ${removed} lines`
    : `+${added} / -${removed}`;

  // Trim the file path to just the last 3 segments for display
  const shortPath = filePath.split('/').slice(-3).join('/');

  return (
    <div className="rounded-md overflow-hidden font-mono text-xs border border-white/10 my-2">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-1.5 bg-white/5 border-b border-white/10">
        <span className="text-blue-400 shrink-0">{isNewFile ? 'Create' : 'Update'}</span>
        <span className="text-white/60 truncate">{shortPath}</span>
        <span className="ml-auto text-white/35 shrink-0 text-[11px]">{summary}</span>
      </div>

      {/* Lines */}
      <div className="bg-[#0d1117] overflow-x-auto">
        {lines.map((line, i) => {
          if (line.kind === 'skip') {
            return (
              <div key={i} className="px-4 py-0.5 text-white/25 select-none">
                ...
              </div>
            );
          }
          return (
            <div
              key={i}
              className={`flex gap-2 px-3 py-[1px] ${
                line.kind === 'add' ? 'bg-green-500/15 text-green-200' :
                line.kind === 'del' ? 'bg-red-500/15 text-red-300' :
                'text-white/50'
              }`}
            >
              <span className="select-none shrink-0 w-3 text-white/25">
                {line.kind === 'add' ? '+' : line.kind === 'del' ? '-' : ' '}
              </span>
              <span className="whitespace-pre">{line.text}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
