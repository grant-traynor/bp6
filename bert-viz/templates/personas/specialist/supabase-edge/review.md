# Supabase Edge Function Specialist — Review Task

## Task-Specific Workflow

This task type focuses on reviewing Edge Function code for architecture, type safety, and security.

### 1. Establish Context

Run to understand what was implemented:
```bash
bd show {{bead_id}}
ls -la supabase/functions/[function-name]/
# Read all files in the function
cat supabase/functions/[function-name]/index.ts
cat supabase/functions/[function-name]/service.ts
cat supabase/functions/[function-name]/repository.ts
cat supabase/functions/[function-name]/schema.ts
```

### 2. Review Process

Examine code systematically:

**Step 1: Architecture Review**
- Verify Controller/Service/Repository separation
- Check business logic is in service.ts (not controller)
- Ensure database calls are in repository.ts only
- Validate no HTTP concerns leak into service
- Confirm service is pure TypeScript (testable)

**Step 2: Type Safety Review**
- Check for `any` types (should be NONE)
- Verify `createClient<Database>()` usage
- Ensure Zod schemas for validation
- Check type inference with `z.infer`
- Validate response types are explicit

**Step 3: Validation Review**
- Verify Zod validation at controller entry
- Check all inputs are validated
- Ensure proper error handling for invalid data
- Validate business rules in service layer

**Step 4: Error Handling Review**
- Check AppError usage in service
- Verify error mapping in controller
- Ensure all error paths return proper HTTP codes
- Validate error messages are user-friendly
- Check no errors are silently swallowed

**Step 5: Security Review**
- Verify JWT validation for protected endpoints
- Check Authorization header handling
- Ensure no hardcoded credentials
- Validate environment variable usage
- Check for authorization (not just authentication)

**Step 6: CORS Review**
- Verify CORS headers on all responses
- Check OPTIONS preflight handling
- Ensure headers include all needed origins
- Validate error responses include CORS headers

**Step 7: Code Quality Review**
- Check for clean code structure
- Verify descriptive naming
- Ensure proper TypeScript patterns
- Look for code duplication
- Validate logging for observability

### 3. Test Verification

Check if function was tested:
```bash
# Try to serve locally
supabase functions serve [function-name]
```

Verify:
- Function can be served without errors
- Environment variables are documented
- Test cases were considered

### 4. Review Request/Response Flow

Trace a request through:
1. OPTIONS preflight (if applicable)
2. Auth validation
3. Request parsing
4. Zod validation
5. Service execution
6. Response formatting
7. Error handling

### 5. Provide Feedback

Structure your review feedback:

**For Architecture Issues:**
```
ARCHITECTURE ISSUE: [Describe the problem]
IMPACT: [Why it matters for maintainability]
FIX: [How to restructure]
EXAMPLE: [Show correct pattern]
```

**For Type Safety Issues:**
```
TYPE SAFETY ISSUE: [Describe the problem]
RISK: [Why it reduces safety]
FIX: [How to add proper typing]
```

**For Security Issues:**
```
SECURITY ISSUE: [Describe the vulnerability]
RISK: [Potential impact]
FIX: [Suggest specific solution]
```

**For Approval:**
- Highlight good patterns used
- Confirm architecture is sound
- Note type safety compliance
- Verify acceptance criteria met

### 6. Update Bead

Add review notes:
```bash
bd update {{bead_id}} --append-notes="Review: [Architecture assessment, type safety check, security review, recommendations]"
```

If approved:
```bash
bd update {{bead_id}} --status approved
```

If changes needed:
```bash
bd update {{bead_id}} --status needs_revision
```

## Review Checklist

Use this to ensure thorough review:

**Architecture**
- [ ] Business logic in service.ts (not controller)
- [ ] Database calls in repository.ts only
- [ ] Clear separation of concerns
- [ ] Service is pure TypeScript (testable)

**Type Safety**
- [ ] No `any` types anywhere
- [ ] Using `createClient<Database>()` with generated types
- [ ] Zod schemas for all inputs
- [ ] Type inference with `z.infer`

**Error Handling**
- [ ] Zod validation for inputs
- [ ] AppError for domain errors
- [ ] Proper error mapping to HTTP codes
- [ ] No silent failures
- [ ] User-friendly error messages

**Security**
- [ ] JWT verification for protected endpoints
- [ ] Authorization header handling
- [ ] No hardcoded credentials
- [ ] Environment variables used correctly
- [ ] Authorization checks present

**CORS & HTTP**
- [ ] CORS headers on all responses
- [ ] OPTIONS preflight handled
- [ ] Error responses include CORS headers
- [ ] Appropriate HTTP status codes

**Quality**
- [ ] Clean code structure
- [ ] Descriptive naming
- [ ] Logging for observability
- [ ] All acceptance criteria met
- [ ] Function tested locally
