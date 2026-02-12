---
name: supabase_sql_rpc_dev_standard
title: Supabase/Postgres Engineering Standards
context: [supabase, postgresql, remote-proceure-call, backend]
description: Mandatory patterns for backend development and code review
capabilities: [review, implement]
priority: mandatory  
triggers:
  extensions: [.sql, .ts]
  paths: ["**/supabase/migrations/*.sql", "**/supabase/functions/**/*.ts"]
  exclude: []
---

# Pairti Backend (Supabase/Postgres) Standards

**STATUS: MANDATORY**
**APPLIES TO:** SQL Migrations, PL/pgSQL RPCs, TypeScript Edge Functions.

---

## 1. 🐘 The Database (PostgreSQL)

**Law: The Database is the Single Source of Truth.**

### Schema & Migrations
*   **No Direct Edits**: AI Agents/Tools NEVER apply migrations directly.
*   **Process**:
    1.  Inspect DB: `supabase gen types typescript` (to understand current state).
    2.  Draft Migration: Create a new file `supabase/migrations/<timestamp>_name.sql`.
    3.  User Review: Ask user to apply via CLI.

### Defensive RPCs (PL/pgSQL)
**Naming Conventions**:
*   **Parameters**: Must start with `p_` (e.g., `p_user_id`).
*   **Variables**: Must start with `v_` (e.g., `v_count`).
*   *Why?* To prevent "Ambiguous Column Reference" errors.

**Robustness**:
*   **JSON Handling**: Never trust JSON inputs.
    ```sql
    -- CORRECT
    COALESCE(p_data->'items', '[]'::jsonb)
    ```
*   **Return Types**: Use `RETURNS TABLE(...)` for explicit schemas. NEVER use `SETOF record`.
*   **Search Path**: `SECURITY DEFINER` functions MUST set `search_path`.

---

## 2. ⚡ Edge Functions (TypeScript)

**Law: The Pure Core.**

### Architecture
*   **`index.ts` (Controller)**:
    *   Handles HTTP Request/Response.
    *   Handles CORS and Auth headers.
    *   Catches Errors -> Maps to HTTP Status Codes.
*   **`service.ts` (Domain Logic)**:
    *   Pure TypeScript. No `Request`/`Response` objects.
    *   Contains business rules.
*   **`repository.ts` (Data Access)**:
    *   Interacts with Supabase Client.

### Validation & Safety
*   **Fail Fast**: Use `zod` to validate inputs immediately in `index.ts`.
    ```typescript
    const payload = RequestSchema.parse(await req.json());
    ```
*   **Type Safety**: Use generated Database types. NO `any`.

### Async & Long-Running Tasks
**Law: User Visibility via TaskLogger.**

*   **Mandate**: Any operation exceeding 2 seconds or processing in the background MUST use `TaskLogger` (`_shared/task_logger.ts`).
*   **Lifecycle**:
    1.  **Init**: `await logger.init('Task Title')` (Creates/Updates `async_tasks` record).
    2.  **Progress**: `await logger.update(percent, 'Message')` (Feedback to UI).
    3.  **Completion**: `await logger.complete(resultMetadata)` (Finalizes state).
    4.  **Failure**: `await logger.fail('User-friendly error')` (Records error).
*   **Why**: Provides real-time status and progress to the frontend user.

---


## 3. 🚫 Common AI Anti-Patterns (Legacy Traps)

**WARNING**: Ignore outdated training data. Adhere to these strict safety rules.

### 🐘 Postgres / SQL
*   **❌ OLD**: `SECURITY DEFINER` (without search_path)
*   **✅ NEW**: `SECURITY DEFINER SET search_path = public`
*   *Why?* Prevents privilege escalation attacks via function hijacking.

*   **❌ OLD**: `SELECT id, name FROM users` (No alias)
*   **✅ NEW**: `SELECT u.id, u.name FROM users u`
*   *Why?* Prevents "Ambiguous Column Reference" when joining tables.

*   **❌ OLD**: `json_data->>'key'` (Blind trust)
*   **✅ NEW**: `COALESCE(json_data->>'key', 'default')`
*   *Why?* SQL hates NULLs. Always provide a fallback.

### ⚡ Edge Functions
*   **❌ OLD**: `createClient(url, key)` (Untyped)
*   **✅ NEW**: `createClient<Database>(url, key)`
*   *Why?* `any` types defeat the purpose of using TypeScript.

*   **❌ OLD**: Handling logic inside the `serve` callback.
*   **✅ NEW**: Delegate immediately to `service.ts`.
*   *Why?* Keeps the entry point clean for CORS and Error handling.

---

## 4. ✅ Code Review Checklist (Self-Correction)

**Target**: Supabase Migrations, RPCs, and Edge Functions.
**Enforcement**: Manual Review + AI Audit.

### 🐘 SQL / RPCs
- [ ] **Prefix Rule**: All RPC params start with `p_`. All local vars start with `v_`.
- [ ] **Search Path**: `SECURITY DEFINER` functions MUST have `SET search_path = public` (or specific schema).
- [ ] **Explicit Table Aliases**: Queries use aliases (`select u.name from users u`), never ambiguous columns.
- [ ] **Defensive JSON**: JSON arrays are wrapped in `COALESCE(x, '[]'::jsonb)`.
- [ ] **Transaction Safety**: Logic that requires atomicity is within a `BEGIN ... END` block.
- [ ] **Return Types**: Uses `RETURNS TABLE` for explicit output schemas.
- [ ] **Indexes**: New foreign keys or frequently queried columns have indexes.
- [ ] **RLS**: Row Level Security policies are enabled on all new tables.

### ⚡ TypeScript Edge Functions
- [ ] **Service Pattern**: Business logic is in `service.ts`, decoupled from `req/res` objects.
- [ ] **Repository Isolation**: DB calls are in `repository.ts`, not mixed with logic.
- [ ] **Zod at the Gate**: Request body is validated with `zod` immediately.
- [ ] **Typed DB Access**: Uses generated Database types (from `supabase gen types`). No `any`.
- [ ] **Http Codes**: Controller maps specific `AppError` types to 400/401/403.
- [ ] **Catch-All**: Top-level `try/catch` ensures no crash goes unlogged.
- [ ] **Task Visibility**: Long-running ops use `TaskLogger` to report progress to `async_tasks` table.
