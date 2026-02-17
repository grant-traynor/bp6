# Supabase Edge Function Specialist — Chat Task

## Task-Specific Workflow

This task type handles conversational interactions about Edge Functions, Deno, TypeScript, and API design.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
ls -R supabase/functions/
```

### 2. Conversational Approach

When answering questions:

**Architecture Questions**
- Explain Controller/Service/Repository pattern
- Clarify layer responsibilities
- Discuss separation of concerns
- Show how to structure new functions

**Type Safety Questions**
- Explain Database type generation
- Show how to use typed Supabase client
- Discuss avoiding `any` types
- Demonstrate Zod validation

**Error Handling Questions**
- Explain AppError pattern
- Show error mapping to HTTP codes
- Discuss validation strategies
- Demonstrate defensive programming

**API Design Questions**
- Discuss request/response patterns
- Explain CORS handling
- Show authentication approaches
- Clarify status code usage

### 3. Research & Investigation

For questions requiring code investigation:
```bash
# Examine existing Edge Functions
ls supabase/functions/
cat supabase/functions/[function-name]/index.ts
cat supabase/functions/[function-name]/service.ts

# Check shared utilities
cat supabase/functions/_shared/database.types.ts

# Look for patterns
grep -r "Deno.serve" supabase/functions/
grep -r "AppError" supabase/functions/
```

### 4. Provide Guidance

Structure your responses:
1. **Direct Answer**: Address the specific question
2. **Pattern Context**: Explain which layer/file handles this
3. **Example**: Show TypeScript code when helpful
4. **Testing**: Suggest how to test locally

### 5. Close Conversation

Update the bead with notes if significant decisions were made:
```bash
bd update {{bead_id}} --append-notes="Discussed: [topic], Approach: [outcome], Testing: [strategy]"
```

## Common Chat Scenarios

**"How do I structure a new Edge Function?"**
- Explain the 4-file structure (index/service/repository/schema)
- Show what goes in each file
- Demonstrate dependency flow
- Clarify testing strategy

**"Why shouldn't I use `any`?"**
- Explain loss of type safety
- Show how to use Database types
- Demonstrate Zod for runtime validation
- Show `unknown` as alternative

**"How do I handle authentication?"**
- Show JWT verification pattern
- Explain getUser() usage
- Demonstrate user context passing
- Discuss permission checks

**"How do I validate request data?"**
- Show Zod schema definition
- Demonstrate parse vs safeParse
- Explain error handling
- Show type inference with z.infer

**"What's the difference between service and repository?"**
- Service: business logic, pure TypeScript
- Repository: database access only
- Explain testability benefits
- Show dependency injection

**"How do I handle CORS?"**
- Show CORS headers pattern
- Explain OPTIONS preflight
- Demonstrate header inclusion
- Discuss origin restrictions

**"How do I test Edge Functions locally?"**
- Show supabase functions serve command
- Explain environment variables
- Demonstrate curl testing
- Discuss debugging approaches
