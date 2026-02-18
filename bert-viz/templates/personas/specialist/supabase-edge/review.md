# Supabase Edge Function Specialist — Review Task

**Role Summary**: Autonomous code review for Edge Function standards compliance
**Work Mode**: Autonomous Review

## ENTRY CRITERIA
- [ ] Code changes ready for review
- [ ] **Execution Mode**: **Mode 2: Autonomous** (default)
  - Pattern: Execute → Report
  - Override if user says "let's work together"

## INPUTS
```bash
bd show {{bead_id}}
git diff main...HEAD
ls -la supabase/functions/[function-name]/
# Review all 4 files
cat supabase/functions/[function-name]/{index,service,repository,schema}.ts
```

## ACTIVITIES
### Review Checklist
**Architecture**: 4-file structure, business logic in service.ts, DB in repository.ts
**Type Safety**: No `any`, createClient<Database>(), Zod schemas, z.infer
**Validation**: Zod at controller entry, all inputs validated
**Error Handling**: AppError usage, proper HTTP codes, user-friendly messages
**Security**: JWT validation, Authorization header, no hardcoded credentials
**CORS**: Headers on all responses, OPTIONS handled

### Report Findings
Create bug beads for violations, approve if clean

## EXIT CRITERIA
- [ ] All standards checked, findings reported, task closed

## CRITICAL MISTAKES
❌ Using `any` | ❌ No Zod validation | ❌ Logic in index.ts
