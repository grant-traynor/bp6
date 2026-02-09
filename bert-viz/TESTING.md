# Regression Testing Guide

## TypeScript → Rust Migration Testing

### Testing Strategy

Since the TypeScript processing code has been migrated to Rust, we validate correctness through:

1. **Manual UI Testing** - Verify visual output matches expectations
2. **Algorithmic Validation** - Code review confirms algorithm correctness
3. **Edge Case Testing** - Test boundary conditions and special cases

### Test Scenarios

#### 1. Basic Tree Building
**Setup:** Simple parent-child relationships
- [ ] Root nodes render at correct level
- [ ] Children nested under parents
- [ ] Orphaned nodes (missing parents) handled gracefully
- [ ] isBlocked flag set correctly for nodes with open blockers

#### 2. Topological Sorting
**Setup:** Beads with blocking dependencies
- [ ] Dependencies sorted correctly (blockers before blocked)
- [ ] Circular dependencies don't cause infinite loops
- [ ] Nodes without dependencies appear first
- [ ] Priority-based tie-breaking works correctly

#### 3. Gantt Layout Calculation
**Setup:** Various dependency configurations
- [ ] X positions calculated from earliest start times
- [ ] Leaf nodes positioned correctly (x = start * 100 + 40)
- [ ] Parent nodes span their children
- [ ] Widths calculated from estimates (width = estimate / 10, min 40)
- [ ] Connectors draw between correct nodes
- [ ] Connector coordinates correct (from predX+width to x at row*48+24)

#### 4. Critical Path Detection
**Setup:** Multiple dependency chains
- [ ] Longest path identified correctly
- [ ] Critical nodes highlighted
- [ ] Critical connectors highlighted
- [ ] Multiple equal paths handled deterministically

#### 5. Filtering
**Setup:** Various filter combinations
- [ ] Text search works (title, id, owner, labels)
- [ ] Hide closed filter works
- [ ] Time-based filters work (1h, 6h, 24h, 7d, 30d, older_than_6h)
- [ ] Hierarchy inclusion preserves ancestors
- [ ] Multiple filters work together correctly

#### 6. State Distribution
**Setup:** Beads spanning multiple time buckets
- [ ] Buckets calculated correctly (100 units pre-zoom)
- [ ] Status counts correct (open, in_progress, blocked, closed)
- [ ] Overlapping beads counted in all relevant buckets
- [ ] Epics and features excluded from counts

#### 7. Collapse/Expand
**Setup:** Tree with multiple levels
- [ ] Collapsed nodes hide children
- [ ] Expanded nodes show children
- [ ] State persists correctly
- [ ] Backend respects collapsed_ids parameter

#### 8. Zoom
**Setup:** Various zoom levels
- [ ] Positions scaled correctly (x * zoom)
- [ ] Widths scaled correctly (width * zoom)
- [ ] Distributions calculated with zoom factor
- [ ] Layout remains consistent across zoom changes

### Edge Cases

#### Empty States
- [ ] No beads: Returns empty tree/layout/distributions
- [ ] No filters: All beads included
- [ ] No dependencies: All nodes at x=0

#### Large Datasets
- [ ] 200+ beads: Performance remains acceptable (<100ms)
- [ ] Deep nesting (10+ levels): Renders correctly
- [ ] Many dependencies: Topological sort completes

#### Invalid Data
- [ ] Missing closed_at timestamps: Handled gracefully
- [ ] Invalid date formats: Parsed without errors
- [ ] Self-referencing dependencies: Detected and handled
- [ ] Duplicate IDs: Prevented by backend

### Data Validation

Test with actual bp6 project data (194 beads):

```bash
# From repo root
bd list --status open
bd show bp6-07y
```

**Validate:**
- Tree structure matches bd tool output
- Blocked nodes correctly identified
- Critical path includes known long chains
- All beads rendered (no missing nodes)

### Automated Testing (Future)

**Rust Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_graph_construction() {
        // Test graph building from beads
    }

    #[test]
    fn test_topological_sort() {
        // Test Kahn's algorithm with various inputs
    }

    #[test]
    fn test_critical_path() {
        // Test longest path finding
    }
}
```

**Frontend Integration Tests:**
- Mock Tauri backend responses
- Verify UI renders correctly
- Test filter interactions
- Validate state management

### Acceptance Criteria

- [ ] All manual test scenarios pass
- [ ] No visual regressions compared to pre-migration
- [ ] No console errors during normal operation
- [ ] Performance targets met (see BENCHMARK.md)
- [ ] Edge cases handled gracefully
- [ ] Real project data (bp6) renders correctly

### Test Execution Log

| Date | Tester | Scenario | Result | Notes |
|------|--------|----------|--------|-------|
| TBD  | -      | -        | -      | Manual testing required |

---

*Last updated: 2026-02-09*
*Test execution pending manual validation*
