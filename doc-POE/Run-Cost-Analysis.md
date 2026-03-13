# POE Run Cost Analysis

**Purpose**: Track token consumption and cost per test run to measure efficiency improvements over time.

**Pricing model**: Anthropic Sonnet 4.6 (as of 2026-03-13)
- Input: $3.00 / M tokens
- Output: $15.00 / M tokens
- Cache write: $3.75 / M tokens
- Cache read: $0.30 / M tokens

---

## Summary Table

| Run | Date | Elapsed | Agents | Output tokens | Cache read tokens | Total cost | Cost/min |
|---|---|---|---|---|---|---|---|
| wordle_004 | 2026-03-12 | 62:17 | 29 | 184,402 | 6,771,135 | **$6.46** | $0.10 |
| wordle_005 | 2026-03-13 | 81:06 | 58 | 485,508 | 22,119,282 | **$20.82** | $0.26 |

---

## wordle_004

**Date**: 2026-03-12
**Run window**: 10:29:45 → 11:32:02 UTC
**Elapsed**: 62:17
**Agents spawned**: 29

### Token Detail

| Token type | Count | Cost |
|---|---|---|
| Input (non-cached) | 222 | $0.00 |
| Output | 184,402 | $2.77 |
| Cache write | 930,784 | $3.49 |
| Cache read | 6,771,135 | $2.03 |
| **Total** | | **$6.46** |

### Notes
- First complete wordle run through poe2 orchestrator
- 15 minutes of lifecycle overhead before first code
- Resume race produced 4 simultaneous Plan Increment agents (root cause of t2 triple-failure)
- Relatively low cache read volume — context was being re-written rather than re-used across agents
- u7s bugs present: resume race, retry race, no review-outcome protocol, no hierarchy close

---

## wordle_005

**Date**: 2026-03-13
**Run window**: 22:24:02 → 23:45:10 UTC
**Elapsed**: 81:06
**Agents spawned**: 58 (agents with usage data)

### Token Detail

| Token type | Count | Cost |
|---|---|---|
| Input (non-cached) | 6,592 | $0.02 |
| Output | 485,508 | $7.28 |
| Cache write | 1,833,849 | $6.88 |
| Cache read | 22,119,282 | $6.64 |
| **Total** | | **$20.82** |

### Notes
- u7s fixes applied (atomic claims, ghost recovery, review-outcome protocol, hierarchy close)
- 58 agents vs 29 — more work attempted (full CONOPS → guardrails → plan → execution → review → rework)
- Cache read 3.3× higher than wordle_004 — T+S+K context bundling working well; context re-used across agents
- Raw input near-zero ($0.02) confirms cache is absorbing context cost effectively
- Output is dominant cost driver ($7.28 of $20.82)
- **Primary cost inefficiencies identified**:
  - `t-anim-test`: 4 failed retries before pass (~$1.50–2.00 wasted) — animation smoke test requires live browser, test skill cannot satisfy headlessly
  - Post-execution review + rework cycle: 22 minutes (~$5–6) — review quality and rework scope drive this
  - Stage gate never enforced — execution ran without human approval, adding unnecessary review cycles

---

## Cost Drivers & Levers

### What drives cost up
1. **Agent retries** — each retry re-reads full context (cache write) and regenerates output. t-anim-test ×4 = ~4× the cost of one pass.
2. **Review/rework cycles** — plan review BLOCKED loops and post-execution rework multiply agent count. Each reviewer is a full agent spawn.
3. **Agent count** — wordle_005 spawned 2× the agents of wordle_004 for proportionally more work.
4. **Output volume** — longer agent outputs (detailed plans, large code files) drive output token cost directly.

### What reduces cost
1. **Prompt caching** — already working well. Cache reads at $0.30/M vs $3.00/M input = 10× saving on context. Increasing cache hit rate further would compound savings.
2. **Skill quality** — better first-pass output means fewer retries and shorter review cycles. The must-not analyst, PM skill, and test skill are the highest-leverage targets.
3. **Correct skill assignment** — sending the wrong skill to a task (e.g. a headless test agent to a browser animation task) wastes an entire agent spawn.
4. **Stage gate enforcement** — blocking execution until the plan is approved prevents speculative work on a flawed plan.

### Projected trajectory
If Phase 4.3 fixes land correctly:
- Stage gate enforcement eliminates speculative execution on bad plans
- ReviewResult artifact paths reduce BLOCKED loop cycles
- Skill fixes reduce retry counts

A well-configured wordle_006 run should be closer to wordle_004 cost ($6–8) with more work completed, targeting <$0.15/min efficiency.

---

## How to Compute Cost for a Run

```bash
cd test_runs/<run>/.poe
sqlite3 dag.db "PRAGMA wal_checkpoint(PASSIVE);"

# Token aggregation across all agent streams
for f in agent_stream/*.jsonl; do
  tail -50 "$f" | python3 -c "
import sys, json
i=o=cc=cr=0
for line in sys.stdin:
    try:
        u = json.loads(line.strip()).get('usage',{})
        i  = max(i,  u.get('input_tokens', 0))
        o  = max(o,  u.get('output_tokens', 0))
        cc = max(cc, u.get('cache_creation_input_tokens', 0))
        cr = max(cr, u.get('cache_read_input_tokens', 0))
    except: pass
if i or o: print(f'{i},{o},{cc},{cr}')
" 2>/dev/null
done | python3 -c "
import sys
ti=to=tcc=tcr=n=0
for l in sys.stdin:
    p=l.strip().split(',')
    if len(p)==4: ti+=int(p[0]);to+=int(p[1]);tcc+=int(p[2]);tcr+=int(p[3]);n+=1
ci,co,ccc,ccr = ti/1e6*3, to/1e6*15, tcc/1e6*3.75, tcr/1e6*0.30
print(f'Agents: {n}  Input: {ti:,}  Output: {to:,}  CacheWrite: {tcc:,}  CacheRead: {tcr:,}')
print(f'Cost: \${ci+co+ccc+ccr:.2f}  (in=\${ci:.2f} out=\${co:.2f} cw=\${ccc:.2f} cr=\${ccr:.2f})')
"
```
