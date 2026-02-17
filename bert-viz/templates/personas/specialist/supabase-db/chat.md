# Supabase Database Specialist — Chat Task

## Task-Specific Workflow

This task type handles conversational interactions about PostgreSQL, RLS, PL/pgSQL, and database design.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
ls -R supabase/migrations/
supabase gen types typescript --local > /tmp/db_types.ts
```

### 2. Conversational Approach

When answering questions:

**Schema Design Questions**
- Discuss table structure and relationships
- Explain RLS policy requirements
- Clarify indexing strategies
- Address performance considerations

**RPC Function Questions**
- Explain naming conventions (p_, v_ prefixes)
- Discuss security considerations (SECURITY DEFINER, search_path)
- Clarify return type patterns
- Show defensive coding patterns

**Migration Questions**
- Explain migration workflow (draft, review, apply)
- Discuss rollback strategies
- Clarify data migration approaches
- Address version control considerations

**Security Questions**
- Explain RLS policies and their purposes
- Discuss function security (DEFINER vs INVOKER)
- Clarify privilege escalation risks
- Show safe patterns

### 3. Research & Investigation

For questions requiring schema investigation:
```bash
# Examine existing migrations
ls -la supabase/migrations/
cat supabase/migrations/[relevant_file].sql

# Check current schema
supabase db diff

# Review existing RPC functions
grep -r "CREATE OR REPLACE FUNCTION" supabase/migrations/
```

### 4. Provide Guidance

Structure your responses:
1. **Direct Answer**: Address the specific question
2. **Security Context**: Explain security implications
3. **Example**: Show SQL code when helpful
4. **Best Practice**: Reference defensive patterns

### 5. Close Conversation

Update the bead with notes if significant decisions were made:
```bash
bd update {{bead_id}} --append-notes="Discussed: [topic], Decision: [outcome], Security considerations: [notes]"
```

## Common Chat Scenarios

**"How do I design table X?"**
- Discuss column types and constraints
- Plan RLS policies needed
- Identify required indexes
- Consider foreign key relationships

**"Why am I getting ambiguous column reference?"**
- Check for missing p_ prefix on parameters
- Verify table aliases are used
- Explain collision between params and columns

**"How do I write a secure RPC function?"**
- Show SECURITY DEFINER with search_path
- Explain parameter validation
- Demonstrate defensive NULL handling
- Clarify privilege requirements

**"What RLS policies do I need?"**
- Analyze data access patterns
- Design policies for SELECT/INSERT/UPDATE/DELETE
- Consider role-based access
- Balance security and usability

**"How do I handle JSON in PostgreSQL?"**
- Show COALESCE for safe key access
- Demonstrate jsonb operators
- Explain type casting
- Show validation patterns
