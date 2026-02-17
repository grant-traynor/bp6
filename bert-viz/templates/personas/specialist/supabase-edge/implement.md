# Supabase Edge Function Specialist — Implement Task

## Task-Specific Workflow

This task type focuses on implementing Supabase Edge Functions with Controller/Service/Repository pattern.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
bd list --status open --parent {{bead_id}}
ls -R supabase/functions/
```

Review:
- Feature description and API requirements
- Request/response schema needs
- Authentication requirements
- Existing function patterns

### 2. Plan Implementation

Before writing code:
- Define request/response types
- Identify business rules (service layer)
- Determine database operations (repository layer)
- Plan error scenarios
- Design validation schema

### 3. Mark Bead In Progress

```bash
bd update {{bead_id}} --status in_progress
```

### 4. Create Function Structure

Create function directory:
```bash
supabase functions new [function-name]
```

Set up file structure:
```
functions/[function-name]/
├── index.ts         # Controller
├── service.ts       # Business logic
├── repository.ts    # Data access
└── schema.ts        # Zod validation
```

### 5. Implementation Steps

**Phase 1: Schema (schema.ts)**
- Define Zod schemas for request validation
- Define response types
- Export inferred TypeScript types
- Include validation rules (min, max, uuid, etc.)

**Phase 2: Repository (repository.ts)**
- Import Database types
- Create repository class
- Implement database operations
- Use typed Supabase client: `createClient<Database>`
- Return typed data or throw errors
- NO business logic here

**Phase 3: Service (service.ts)**
- Import types from schema
- Import repository interface
- Implement business logic
- NO HTTP Request/Response handling
- NO direct database access
- Pure TypeScript, fully testable
- Use AppError for domain errors

**Phase 4: Controller (index.ts)**
- Set up CORS headers
- Handle OPTIONS preflight
- Validate JWT and get user
- Parse and validate request with Zod
- Instantiate repository and service
- Call service methods
- Map errors to HTTP status codes
- Return JSON responses with CORS headers

### 6. Error Handling

Create AppError class if not exists:
```typescript
export class AppError extends Error {
  constructor(
    message: string,
    public statusCode: number,
    public code?: string
  ) {
    super(message);
    this.name = 'AppError';
  }
}
```

Map errors in controller:
- AppError → use statusCode from error
- Validation errors → 400
- Auth errors → 401
- Permission errors → 403
- Not found → 404
- Unexpected errors → 500

### 7. Type Safety

Ensure:
- Use `createClient<Database>()` with generated types
- Never use `any` type
- Use Zod for runtime validation
- Use `z.infer` for type inference
- Handle all error cases explicitly

### 8. Testing

Test locally before completing:
```bash
supabase functions serve [function-name]
```

Test with curl:
```bash
curl -i --location --request POST 'http://localhost:54321/functions/v1/[function-name]' \
  --header 'Authorization: Bearer [token]' \
  --header 'Content-Type: application/json' \
  --data '{"field": "value"}'
```

### 9. Update Type Definitions

If database schema changed:
```bash
supabase gen types typescript --local > supabase/functions/_shared/database.types.ts
```

### 10. Update Bead

Document what was done:
```bash
bd update {{bead_id}} --notes="[Function summary, endpoints, auth requirements]"
bd update {{bead_id}} --design="[Architecture used, validation approach, error handling strategy]"
```

### 11. Close Bead

```bash
bd close {{bead_id}} --reason="[What was implemented, how it meets requirements]"
```

## Implementation Checklist

Before closing:
- [ ] Service pattern: Business logic in service.ts, decoupled from HTTP
- [ ] Repository isolation: All database calls in repository.ts
- [ ] Zod at the gate: Request validation in controller
- [ ] Typed DB access: Using `createClient<Database>()`
- [ ] No `any` types anywhere
- [ ] Error mapping: Controller maps to HTTP codes
- [ ] CORS headers: All responses include CORS
- [ ] Auth check: Protected endpoints verify JWT
- [ ] Tested locally: Function works with `supabase functions serve`
- [ ] Environment vars: No hardcoded credentials
- [ ] Logging: Long operations have logging
