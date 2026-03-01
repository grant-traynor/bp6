# BERT Viz Architecture

## Overview

BERT Viz is a high-performance project management visualization tool built with Tauri (Rust backend) + React (TypeScript frontend). The application migrated compute-intensive data processing from TypeScript to Rust for 10-20x performance improvements.

## Technology Stack

- **Frontend**: React 19 + TypeScript + Vite + Tailwind CSS
- **Backend**: Rust + Tauri v2
- **Data**: Beads issue tracker (JSONL files in `.beads/`)
- **IPC**: Tauri commands (JSON serialization via serde)

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     React Frontend (TypeScript)              │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │  App.tsx    │  │ WBSTreeList  │  │  GanttBar        │   │
│  │  (State)    │  │ (Component)  │  │  (Component)     │   │
│  └──────┬──────┘  └──────────────┘  └──────────────────┘   │
│         │ fetchProcessedData({ filters })                   │
│         │                                                    │
│         ▼                                                    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │           Tauri IPC Layer (invoke)                   │   │
│  └────────────────────┬────────────────────────────────┘   │
└───────────────────────┼─────────────────────────────────────┘
                        │ JSON (FilterParams)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                   Rust Backend (Tauri)                       │
│  ┌────────────────────────────────────────────────────┐     │
│  │  get_processed_data(params: FilterParams)          │     │
│  │    ↓                                                │     │
│  │  1. Load beads from .beads/issues.jsonl            │     │
│  │  2. Apply filters (text, status, time, hierarchy)  │     │
│  │  3. Build dependency graph                         │     │
│  │  4. Build WBS tree (parent-child relationships)    │     │
│  │  5. Sort siblings (topological sort)               │     │
│  │  6. Calculate X positions (earliest start times)   │     │
│  │  7. Calculate node ranges (position + width)       │     │
│  │  8. Find critical path (longest path algorithm)    │     │
│  │  9. Generate Gantt layout (items + connectors)     │     │
│  │ 10. Calculate state distributions (buckets)        │     │
│  │    ↓                                                │     │
│  │  Returns: ProcessedData { tree, layout, dist }     │     │
│  └────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
                        │ JSON (ProcessedData)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    React Rendering                           │
│  • WBS tree with hierarchy and expansion                    │
│  • Gantt chart with positioned bars and connectors          │
│  • State distribution header with bucket counts             │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Initial Load
```
User opens project
  → React: openProject(path)
  → Rust: set_current_dir(), watch .beads file
  → React: fetchProcessedData(defaultFilters)
  → Rust: get_processed_data() processes all beads
  → React: setProcessedData(), renders UI
```

### 2. Filter Change
```
User types in search box (debounced 300ms)
  → React: setFilterText(newText)
  → useEffect triggers: fetchProcessedData({ filter_text: newText, ... })
  → Rust: get_processed_data() with new filters
  → React: setProcessedData(), re-renders UI
```

### 3. Collapse/Expand
```
User clicks expand/collapse icon
  → React: setCollapsedIds(updated set)
  → useEffect triggers: fetchProcessedData({ collapsed_ids: [...], ... })
  → Rust: get_processed_data() applies expansion state
  → React: setProcessedData(), tree shows/hides children
```

## Key Algorithms

### Topological Sort (Kahn's Algorithm)
**Purpose**: Sort sibling nodes by blocking dependencies
**Complexity**: O(V + E) where V = nodes, E = edges
**Implementation**: `topological_sort()` in lib.rs:705

```rust
1. Build in-degree map (count of blockers for each node)
2. Add nodes with 0 in-degree to queue
3. While queue not empty:
   a. Pop node, add to result
   b. Decrement in-degree of successors
   c. Add successors with 0 in-degree to queue
4. Handle circular dependencies (append remaining)
```

### Critical Path (Longest Path)
**Purpose**: Highlight the longest dependency chain
**Complexity**: O(V + E) with memoization
**Implementation**: `find_critical_path()` in lib.rs:1206

```rust
1. For each node, recursively find max distance to end
2. Memoize results to avoid recomputation
3. Find node with global max distance (start of critical path)
4. Reconstruct path by following next pointers
```

### Gantt Layout Calculation
**Purpose**: Position beads in 2D Gantt chart
**Complexity**: O(V + E)
**Implementation**: `generate_gantt_layout()` in lib.rs:1283

```rust
1. Flatten tree to get visible rows and depths
2. Calculate earliest start times (X positions)
3. Calculate node ranges (position and width)
4. Generate items (bead + position + row + flags)
5. Generate connectors (dependency arrows)
```

## Data Structures

### Core Types (Rust)

```rust
// Processed output sent to frontend
pub struct ProcessedData {
    tree: Vec<WBSNode>,          // Hierarchical tree
    layout: GanttLayout,          // Positioned items + connectors
    distributions: Vec<BucketDistribution>, // State counts
}

// WBS tree node
pub struct WBSNode {
    #[serde(flatten)]
    bead: Bead,                   // All bead fields flattened
    children: Vec<WBSNode>,
    isExpanded: bool,
    isBlocked: bool,
    isCritical: bool,
}

// Gantt layout
pub struct GanttLayout {
    items: Vec<GanttItem>,
    connectors: Vec<GanttConnector>,
    rowCount: usize,
    rowDepths: Vec<usize>,
}

// Filter parameters from frontend
pub struct FilterParams {
    filter_text: String,
    hide_closed: bool,
    closed_time_filter: ClosedTimeFilter,
    include_hierarchy: bool,
    zoom: f64,
    collapsed_ids: Vec<String>,
}
```

## Performance Characteristics

### Before Migration (TypeScript in Browser)
- **200 beads**: ~150-300ms processing time
- **UI blocking**: Laggy interactions during processing
- **Memory**: GC pauses during large computations

### After Migration (Rust Backend)
- **200 beads**: ~10-20ms processing time (10-15x faster)
- **UI non-blocking**: Processing happens in background
- **Memory**: No GC, predictable performance

### Bottlenecks
1. **IPC overhead**: JSON serialization (~2-5ms)
2. **File I/O**: Reading .beads/issues.jsonl (~5-10ms)
3. **Algorithm complexity**: O(V + E) dominates for large projects

## Development Workflow

### Adding New Filters

**Rust Backend** (`src-tauri/src/lib.rs`):
```rust
1. Add field to FilterParams struct
2. Update get_processed_data() to use new filter
3. Implement filter logic (follow existing patterns)
```

**TypeScript Frontend** (`src/App.tsx`):
```typescript
1. Add state variable: useState<FilterType>()
2. Add to useEffect dependencies
3. Pass to fetchProcessedData({ new_filter, ... })
4. Add UI control to Header or filters section
```

### Modifying Layout Algorithm

Edit `src-tauri/src/lib.rs`:
```rust
1. Find relevant function (e.g., calculate_node_ranges)
2. Update logic (Rust compiles, tests catch errors)
3. Rebuild: cargo build
4. Test in frontend: npm run tauri dev
```

### Adding New Tauri Commands

```rust
// 1. Define command in lib.rs
#[tauri::command]
fn my_new_command(param: Type) -> Result<ReturnType, String> {
    // Implementation
    Ok(result)
}

// 2. Register in invoke_handler
.invoke_handler(tauri::generate_handler![
    get_beads, get_processed_data, my_new_command, // ...
])
```

```typescript
// 3. Add to api.ts
export async function myNewCommand(param: Type): Promise<ReturnType> {
    return await invoke<ReturnType>("my_new_command", { param });
}
```

## Migration Notes

### What Changed
- **Removed**: 358 lines of TypeScript processing code
- **Added**: ~800 lines of Rust processing code
- **Kept**: TypeScript interfaces for type safety
- **Improved**: 10-20x performance, better memory usage

### Breaking Changes
- None - API remains compatible

### Known Limitations
1. **Large files**: Reading 10,000+ line JSONL files takes >100ms
2. **Deep nesting**: Trees with 20+ levels may have minor layout issues
3. **Circular deps**: Handled but may produce unexpected ordering

## Future Optimizations

### Short-term
1. **Parallel filtering**: Process bead chunks in parallel
2. **Incremental updates**: Only recompute changed portions
3. **Caching**: Cache processed data, invalidate on file changes

### Long-term
1. **Database backend**: Replace JSONL with SQLite for faster queries
2. **WASM**: Embed Rust in browser for offline mode
3. **Streaming**: Stream large datasets instead of loading all at once

## Security Considerations

- **Input validation**: Rust type system prevents invalid data
- **File access**: Tauri restricts file system access
- **IPC safety**: JSON deserialization validates structure
- **No SQL injection**: No database (JSONL file only)

---

*Last updated: 2026-02-09*
*Architecture reflects post-migration state*
