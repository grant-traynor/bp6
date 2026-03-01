# QC Engineer — Generate Report

**Role Summary**: Autonomous generation of quality metric reports and trend analysis
**Work Mode**: Autonomous Reporting

---

## ENTRY CRITERIA

- [ ] Bead assigned for report generation
- [ ] Bead status: open
- [ ] **Execution Mode**: **Mode 2: Autonomous** (default)
  - Pattern: Execute → Report
  - Override if user says "let's work together"
- [ ] Metrics data collected (exists in `.metrics/collected/`)
- [ ] Report scope defined (snapshot vs. trend, specific persona vs. project-wide)

---

## INPUTS

### Context Establishment Protocol (C-E-P)

**Step 1: Read Report Bead**
```bash
bd show {{bead_id}}
```
**Extract**: What type of report? (snapshot, trend, comparison?) What scope? (persona, subsystem, project-wide?) Who's the audience? (QA Engineer, user, team?)

**Step 2: Check Available Data**
```bash
# List collected metrics
ls -lt .metrics/collected/

# Check data completeness
cat .metrics/collected/*/{{scope}}.jsonl | jq '.data | keys'
```

**Step 3: Identify Reporting Period**
```bash
# Get earliest and latest collection timestamps
ls .metrics/collected/ | sort | head -1  # earliest
ls .metrics/collected/ | sort | tail -1  # latest
```

**Step 4: Read Metric Definitions** (for context)
```bash
# Get targets/thresholds from templates
grep -A 30 "^## MEASUREMENTS" bert-viz/templates/personas/{{scope}}/*.md
```

---

## ACTIVITIES

### Phase 1: Preparation

**1.1. Mark Bead In Progress**
```bash
bd update {{bead_id}} --status in_progress
```

**1.2. Load Collected Data**

```bash
# Load all relevant metric collections
cat .metrics/collected/*/{{scope}}.jsonl > .metrics/working/data.jsonl
```

**1.3. Determine Report Type**

Based on bead requirements:
- **Snapshot**: Current state of metrics (single point in time)
- **Trend**: Historical comparison (multiple time points)
- **Comparison**: Across personas/subsystems (e.g., Flutter vs. Supabase quality)
- **Anomaly**: Highlight metrics outside target ranges

---

### Phase 2: Report Generation

**2.1. Calculate Aggregate Metrics**

**For Snapshot Reports:**
```markdown
## Quality Metrics Report - {{scope}}
**Generated**: {{timestamp}}
**Period**: {{date_range}}

### Process Metrics
| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Time to Context | 4.2 min | < 5 min | ✅ PASS |
| Implementation Time | 2.3 hrs | varies | ℹ️ INFO |

### Quality Metrics
| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Tests Passing | 98% | 100% | ⚠️ BELOW |
| Linter Clean | 2 warnings | 0 | ⚠️ BELOW |
| AC Met | 100% | 100% | ✅ PASS |

### Outcome Metrics
| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Rework Rate | 15% | < 10% | ⚠️ ABOVE |
| Completion Rate | 87% | > 80% | ✅ PASS |
```

**For Trend Reports:**
```markdown
## Quality Trend Report - {{scope}}
**Generated**: {{timestamp}}
**Period**: {{start_date}} to {{end_date}}

### Tests Passing % (over time)
| Date | Value | Trend |
|------|-------|-------|
| 2025-01-15 | 95% | ⬇️ -3% |
| 2025-01-22 | 97% | ⬆️ +2% |
| 2025-02-01 | 98% | ⬆️ +1% |

**Trend**: Improving (95% → 98% over 3 weeks)

### Rework Rate % (over time)
| Date | Value | Trend |
|------|-------|-------|
| 2025-01-15 | 12% | — |
| 2025-01-22 | 18% | ⬆️ +6% |
| 2025-02-01 | 15% | ⬇️ -3% |

**Trend**: Volatile, above target (target: < 10%)
```

**2.2. Identify Anomalies**

Flag metrics outside target ranges:
```markdown
## Anomalies Detected

### ⚠️ High Priority
- **Rework Rate (15%)**: Exceeds target of < 10%
  - Recommended action: Escalate to QA Engineer for root cause analysis
  - Data source: beads reopened / total closed

### ⚠️ Medium Priority
- **Linter Warnings (2)**: Target is 0 warnings
  - Recommended action: Run `flutter analyze` and address
  - Data source: flutter analyze output

### ✅ Low Priority
- **Tests Passing (98%)**: Slightly below 100% target
  - Recommended action: Investigate 2% test failures
  - Data source: test output
```

**2.3. Generate Visualizations** (if applicable)

```bash
# Simple ASCII charts for trends
# Example: Rework rate over time
#
# 20% │     ╭─╮
# 15% │   ╭─╯ ╰─╮ ← Current
# 10% ├───┼───┼───┼─── Target
#  5% │
#     └───┴───┴───┴───
#      W1  W2  W3  W4
```

**2.4. Add Recommendations for QA**

```markdown
## Recommendations for QA Engineer

### High Impact
1. **Investigate Rework Rate (15%)**
   - Hypothesis: Unclear acceptance criteria or incomplete design?
   - Suggested audit: Review beads with status=reopened, analyze patterns

2. **Address Linter Warnings**
   - Quick win: Run and fix linter issues
   - Consider adding pre-commit hook

### Medium Impact
3. **Test Coverage Gaps**
   - 2% of tests failing
   - Review failed test output to identify patterns
```

---

### Phase 3: Documentation & Distribution

**3.1. Store Report**

```bash
# Create reports directory
mkdir -p .metrics/reports

# Save markdown report
cat > .metrics/reports/{{timestamp}}_{{scope}}.md << 'EOF'
[generated report content]
EOF
```

**3.2. Update Bead with Notes**

```bash
bd update {{bead_id}} --notes="Generated {{report_type}} report for {{scope}}.

Key findings:
- {{count_pass}} metrics meeting targets
- {{count_warn}} metrics below/above targets
- {{count_anomalies}} anomalies requiring QA attention

Report available: .metrics/reports/{{timestamp}}_{{scope}}.md

Recommendations for QA Engineer:
- {{top_recommendation_1}}
- {{top_recommendation_2}}"
```

**3.3. Close Bead**

```bash
bd close {{bead_id}} --reason="Report generation complete for {{scope}}. {{total_metrics}} metrics analyzed, {{anomalies}} anomalies flagged. Report stored and ready for QA review."
```

**3.4. Optional: Notify QA Engineer**

If anomalies require attention:
```bash
# Create a bead for QA Engineer to investigate
bd create \
  --type=task \
  --title="Investigate quality anomalies: {{scope}}" \
  --assignee=qa-engineer \
  --priority=2 \
  --description="QC report flagged {{count_anomalies}} metrics outside targets. See .metrics/reports/{{timestamp}}_{{scope}}.md for details."
```

---

## MEASUREMENTS

### Process Metrics
- **Report generation time**: < 10 minutes for snapshot, < 30 minutes for trend
- **Data points analyzed**: Count of metric values processed

### Quality Metrics
- **Calculation accuracy**: 100% (all formulas verified)
- **Anomaly detection**: 0 false positives (only flag true out-of-range values)
- **Recommendation relevance**: Qualitative - are suggestions actionable?

### Outcome Metrics
- **QA action rate**: % of flagged anomalies that result in QA investigation
- **Metric improvement**: Do flagged metrics improve in subsequent collections?

---

## OUTPUTS

- **Quality report**: Markdown file with metrics, trends, anomalies
- **Recommendations**: Actionable suggestions for QA Engineer
- **Data visualizations**: Trends over time (if applicable)
- **QA bead** (optional): Created for high-priority anomalies

---

## EXIT CRITERIA

- [ ] Report generated with all collected metrics
- [ ] Trends calculated (if applicable)
- [ ] Anomalies flagged with thresholds
- [ ] Recommendations provided for QA
- [ ] Report stored in `.metrics/reports/`
- [ ] Bead closed with summary

---

## CRITICAL MISTAKES TO AVOID

### ❌ Mistake #1: Doing Root Cause Analysis

**WRONG**:
```markdown
## Analysis
Rework rate is high because developers aren't reading AC carefully.
This is caused by rushed sprint planning and lack of design reviews.

Recommendation: Institute mandatory design review before implementation.
```

**CORRECT**:
```markdown
## Anomalies
- Rework Rate: 15% (target: < 10%)
- Status: ⚠️ ABOVE TARGET

Recommendation for QA: Investigate root cause of high rework rate.
Suggested starting points: Review beads with status=reopened, analyze common patterns.
```

**Why**: QC measures and reports. QA analyzes and proposes solutions.

---

### ❌ Mistake #2: Ignoring Target Thresholds

**WRONG**: "Tests passing: 98% - Great job team!"
[Template says target is 100%, not 98%]

**CORRECT**: "Tests passing: 98% (target: 100%) - Status: ⚠️ BELOW TARGET by 2%"

**Why**: Targets are defined in templates. Always compare against them.

---

### ❌ Mistake #3: Reporting Without Data

**WRONG**: "Code quality is excellent based on my assessment"
[No data collected]

**CORRECT**: "Unable to generate report - no metrics collected. Create `collect-metrics` bead first."

**Why**: Reports must be data-driven, not subjective.

---

## REPORT TEMPLATES

### Snapshot Report Template
```markdown
# Quality Metrics Report - {{scope}}
**Generated**: {{timestamp}}
**Period**: {{date_range}}

## Executive Summary
- {{count_metrics}} metrics analyzed
- {{count_pass}} meeting targets ({{percentage_pass}}%)
- {{count_warn}} outside targets ({{percentage_warn}}%)
- {{count_anomalies}} requiring QA attention

## Process Metrics
[table with: Metric | Current | Target | Status]

## Quality Metrics
[table with: Metric | Current | Target | Status]

## Outcome Metrics
[table with: Metric | Current | Target | Status]

## Anomalies
[list with: metric, severity, recommendation]

## Recommendations for QA Engineer
[prioritized list of suggested investigations]
```

### Trend Report Template
```markdown
# Quality Trend Report - {{scope}}
**Generated**: {{timestamp}}
**Period**: {{start_date}} to {{end_date}}

## Trending Up ⬆️ (Improving)
- {{metric_1}}: {{start_value}} → {{end_value}} (+{{delta}})

## Trending Down ⬇️ (Degrading)
- {{metric_2}}: {{start_value}} → {{end_value}} (-{{delta}})

## Stable ➡️
- {{metric_3}}: {{value}} (±{{variance}})

## Detailed Trends
[time-series table for each metric]

## Recommendations for QA Engineer
[focus on degrading trends and volatile metrics]
```
