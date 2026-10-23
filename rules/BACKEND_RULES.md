# Backend Rules

Language-agnostic engineering rules for backend services. Written as principles so they can be reused across projects and languages; examples are short pseudocode. Anything tied to a specific stack lives at the end under **Project-specific notes**.

## Table of Contents
1. [Foundations](#1-foundations)
2. [Architecture & Design](#2-architecture--design)
3. [Services & Logging](#3-services--logging)
4. [Data & Queries](#4-data--queries)
5. [Security](#5-security)
6. [Performance & Memory](#6-performance--memory)
7. [Code Quality](#7-code-quality)
8. [Git & Commits](#8-git--commits)
9. [Project-specific notes](#9-project-specific-notes)

---

## 1. Foundations

### Layered architecture
Dependencies point **inward only** — inner layers never depend on outer layers.

```
Domain (entities, interfaces)  →  Infrastructure (repositories/DB)  →  Services (business logic)  →  Controllers (transport)  →  Routes (input DTOs, output presenters, validation)
```

- **Routes**: input validation/DTOs, output presenters.
- **Controllers**: transport (HTTP) handling only — call a service, return its result. No business logic.
- **Services**: business logic.
- **Repositories**: data access only.
- **Interfaces**: contracts the outer layers depend on (not concrete classes).

---

## 2. Architecture & Design

### SOLID
- **SRP** — one reason to change per class/service. Don't mix concerns (e.g. creating a record + sending email + analytics in one service → split into three).
- **OCP** — extend by adding new implementations, not editing existing code (inject a strategy/processor interface).
- **LSP** — every implementation of an interface must be substitutable for it (all honor the same contract).
- **ISP** — prefer several focused interfaces over one fat interface (e.g. separate read vs write repositories) so consumers don't depend on methods they don't use.
- **DIP** — depend on abstractions, not concretions. Inject dependencies; never `new` a concrete repository inside a service.

### Clean Architecture — dependency rule
Source dependencies point inward. Domain has no dependencies; interfaces depend only on domain; infrastructure implements interfaces; services depend on interfaces; controllers depend on services; routes are outermost.

**Adding a feature (order):** (1) define the interface if new, (2) implement the repository, (3) write the service (business logic, validation, error handling), (4) add a thin controller, (5) wire the route with input validation + output presenter.

### Design patterns (as used here)
- **Repository** — one type per aggregate, behind an interface; the only place that touches the DB. (Rules: see §4.)
- **Template Method / ServiceBase** — a base class defines `run()` (subclass implements the logic) and `execute()` (runs it, collects errors, returns a uniform result). (See §3.)
- **Factory** — centralize construction when the concrete type is chosen at runtime.
- **Strategy** — swap interchangeable algorithms (e.g. per-field validators) behind a common interface.
- **Dependency Injection** — pass collaborators in (constructor/args); optionally a container resolves them. Never hard-wire concretes.

### Service organization
- **Entity-creation services** → per-entity directory: `services/<entity>/create<Entity>`.
- **Junction / linking services** (many-to-many) → the related entity's `helper/` subdir: `services/<entity>/helper/link<Entity>With<Target>` — so they're reusable across workflows.
- **Workflow-specific services** → stay in their feature directory.

---

## 3. Services & Logging

### Service structure
- Services extend the base class and implement `run()`. The base's `execute()` runs it, collects errors, and returns a uniform result — used **only by controllers**. Service-to-service calls use `run()` (so the caller controls error handling).
- **Signature convention:** `context` first, `args` second. Read `this.context` / `this.args` inside.
- **Validate at the boundary.** Route-level validation covers controller-invoked services, so those don't need their own schema. A service invoked by another service (via `run()`) bypasses route validation, so it **must** validate its own inputs.
- **Normalize/trim inputs before any processing** — trim strings, lowercase emails, canonicalize identifiers — *before* uniqueness checks, lookups, or persistence, so every downstream step sees the clean value.
- Report failures with the error mechanism (§ error handling), not ad-hoc throws or returned error objects. No `try/catch` inside `run()` except around a genuine integration boundary.

### Error handling
- Define every error in **one registry**, each with: `name`, `statusCode`, an "operational vs bug" flag, user-facing `description`, and a unique numeric `code`.
- Services raise an error by its **name (string)** — they do **not** import the registry. The transport layer maps the name to the full error object and shapes the wire response.
- A service may raise multiple errors; if any exist, it fails. Error names are PascalCase and must exist in the registry — no ad-hoc errors.

### Logging discipline
Logs must give a full trail when something fails in production. Every call boundary that produces a value gets a log line.

- **Single line per event.** The object literal may wrap for readability, but it's one logical statement — never split one event across multiple log calls.
- **`message`** is a short, purposeful, human-readable sentence naming what just happened. **`context`** carries the structured data.
- **Log at every call boundary that returns a value:** after each repository call (logged by the *caller* — repos don't log, see §4), after each service-to-service call, after each helper/method call that returns a value. Also log: service entry (with inputs), every create/update/delete, and all errors (with full context/exception).
- **Result-log hugs the call** — the log recording a call's return sits on the very next line, no blank line between them. Blank lines separate logical phases, not a call from its result log.
- **Log the whole returned value** under one key — don't cherry-pick sub-fields. For parallel calls that resolve several values, log them together, one whole value per key.
- **Every `context` value is `stringify(rawValue)` and nothing else** — no `.toISOString()`, `.toString()`, `String()`, template strings, or arithmetic first. A safe stringify never throws on null/undefined; transforms crash exactly when something's already wrong and you most need the log.

---

## 4. Data & Queries

### Repositories
- **Data access only** — no business logic, no logging (the calling service logs the result), no cross-entity orchestration.
- **Return plain objects**, never ORM model instances (convert if needed) — so nothing downstream depends on the ORM.
- **Read methods take identifying args first, then a single defaulted `options` object** — `limit`, `offset`, `order`, `attributes`, each with an **inline default chosen for that method** (e.g. a "by N ids" method defaults `limit` to the id count; a list method defaults to a page size). Don't hoist these to shared module constants — inline defaults keep each method's intent in the reader's eye line. Make `attributes` defaulted (explicit projection) rather than "all columns".

### Reading
- **Select only the fields you need** — never fetch whole rows by default.
- **Avoid N+1** — eager-load related data in one query instead of looping.
- **Paginate** list endpoints (`limit`/`offset`, return total + page info).
- **Ask the DB the question** — use `count`/aggregates; don't fetch rows to count them in app code.

### Writes & transactions
- Wrap **multi-step related writes in a transaction**; roll back on failure. Never create parent + children (or move balances, etc.) without one.

### Connections
- Use a **connection pool** with sane max/min/acquire/idle limits; disable verbose query logging in production.

### Migrations — app-level relationships, DB-level invariants
- **No FK references / no CASCADE** in schema — referential integrity and cascading are handled in the service layer (full control, easier to debug). FK columns declare only type + nullability.
- **No indexes added solely to speed up FK lookups.**
- **Unique constraints ARE allowed and expected** to enforce invariants (composite junction keys, globally-unique columns). The DB enforces them; app code can rely on them (race-safe `findOrCreate`, etc.).
- **Editing schema:** for an existing (already-migrated) table, edit its migration for fresh deploys and provide the equivalent `ALTER` for already-migrated DBs; for a genuinely new table, add a new migration.

---

## 5. Security

- **Validate at route AND service level.** Route validation checks shape; service validation enforces business rules (uniqueness, etc.). Normalize/trim first (§3).
- **Prevent SQL injection** — always parameterize (ORM where-clauses or bound `:params`). Never string-concatenate user input into queries.
- **Prevent XSS** — sanitize/escape output (especially anything rendered), typically in the presenter.
- **AuthN/AuthZ** — verify the token/identity, then check the specific permission for the action; deny by default.
- **Hash passwords** with a strong adaptive hash (bcrypt/argon2) + salt; never store or log plaintext.
- **Secrets from environment/secret store** — never hard-code secrets.
- **Rate-limit** sensitive endpoints (login, etc.).
- **Sanitize error messages to clients** — generic user-facing text; never leak internals, queries, or which field failed for auth.

---

## 6. Performance & Memory

- **Avoid memory leaks** — bound caches (max size / eviction / TTL); clean up timers, listeners, and resources.
- **Stream large datasets** instead of loading everything into memory.
- **Cache** hot reads (e.g. Redis) with a TTL, and **invalidate on write**.
- **Import only what you use** (avoid pulling whole utility libraries).
- **Offload CPU-intensive work** to worker threads/processes.
- **Do slow/async work in background jobs/queues** with retries + backoff — the request path just enqueues.
- **Don't block the event loop** — chunk long synchronous loops / yield.
- **Graceful shutdown** — on SIGTERM/SIGINT stop accepting requests, drain, close DB/cache/queue connections, then exit.

---

## 7. Code Quality

### Simplicity & readability
- **Self-documenting names** — names state intent; if you need a comment to explain code, refactor instead.
- **Single-purpose functions** — one job each; compose them.
- **Early returns** over deep nesting.
- **`async/await`** over nested callbacks.
- **Consistent error handling** — one mechanism throughout (don't mix throwing and returning error objects).
- **No magic numbers/strings** — use named constants; **use named status/enum constants instead of raw string literals**, and convert neighbouring literals when you touch them.
- **Keep a call on one line** unless it's long (~200 chars), then break the arguments; don't multi-line short calls.
- **A response-shape change is a full loop** — any added/renamed/removed response field must update the **service, the presenter, and the route/response schema together** (a stale schema silently strips the field).
- **No duplication** — extract shared logic.

### Anti-patterns to avoid
- **God classes/services** — split by responsibility.
- **Business logic in controllers** — controllers stay thin.
- **Silent error swallowing** — never `catch` and ignore; log + surface.
- **Related writes without a transaction.**
- **Tight coupling** — go through interfaces/repositories, not concrete models, inside services.

### Code review checklist
- **Security:** inputs validated (route + service); parameterized queries; output sanitized; passwords hashed; secrets in env; authN/authZ enforced; rate limiting on sensitive routes; error messages don't leak internals.
- **Performance:** select only needed fields; no N+1; pagination on large sets; transactions for related writes; pooling configured; caching where useful; no event-loop blocking.
- **Architecture:** clean-architecture layers respected; services use the base class; repositories behind interfaces and return plain objects; thin controllers; input DTOs + output presenters; dependencies injected; SOLID followed.
- **Error handling:** all errors defined in the registry; raised by PascalCase string name; registry not imported into services; transport maps names to wire; each error has name/status/operational/description/code.
- **Code quality:** clear names; no needless comments; single responsibility; early returns; consistent async + error handling; no magic numbers; no duplication.
- **Logging:** every call-boundary result logged; entry logged with inputs; changes logged; errors logged with context; consistent format; no sensitive data logged.
- **Testing:** dependencies injectable; no tight coupling to externals; pure functions where possible; error + edge cases handled.

### New-feature checklist
Interface (if new) → repository → service (logic + validation + error handling + logging) → thin controller → route with input validation + output presenter.

---

## 8. Git & Commits

- **Never run `git commit` / `push` / `rebase` / open a PR on the user's behalf.** When asked for commits, return a **written proposal**: one section per commit with a subject (imperative, `type(scope): …`), a short *why* body, and the file list; group related changes. The user runs them or explicitly authorizes. Read-only git (`status`/`log`/`diff`) is fine without asking.
- Commit in **logical groups**, one concern per commit — not one giant commit.

---

## 9. Project-specific notes

FSG stack specifics (Node.js + Express + Sequelize + AJV). These are the concrete forms of the universal rules above; keep them here so the rules stay portable.

### Stack
Node.js + Express; Sequelize ORM; Clean Architecture.

### Sequelize reads
- Read with `raw: true, nest: true` to get plain objects (handles joins). When an instance is unavoidable, return `instance.toJSON()`.
- Repositories stay query-only and contain **no `logger.*`** — the calling service logs the result.

### Models declare only the attributes the project uses
Both backends read from the **same DB**, and each defines its own Sequelize models. A model declares **only the columns that project actually queries** — not the full table schema. Internal-only workflow columns don't belong in the external (client) model, and vice-versa. When a migration adds a column, add it to a project's model **only if that project reads it** (e.g. `no_fetal_gender_pdf_file_path` goes on the external `DocInstance` model because the client download reads it, but internal-only fields like `manual_field_values` stay off it). Keeps consumer models lean and free of dead fields.

### Error handling (concrete)
- Registry: `src/libs/errorTypes.js`. Services call `this.addError('SomeErrorType')` (string name, PascalCase — enforced by ServiceBase); never import `errorTypes`.
- The response helper (`src/helpers/response.helpers.js`) extracts the names from service errors and maps them to the registry objects for the wire response.
- Error object shape: `{ name, statusCode (from http-status-codes), isOperational, description, errorCode }`.

### Logging (concrete)
Logger at `lib/logger.js`. Info: `logger.info('ServiceName: ', { message, context: { key: JSON.stringify(v) } })`. Error: `logger.error('ServiceName: ', { message: \`… ${error.message}\`, exception: error })`. First log in `run()` uses `{ args: JSON.stringify(this.args) }`.

### Repository read-method `options` convention
Second parameter is `options = {}` (named `options`, **not** `opts`). Destructure `limit`, `offset`, `order`, `attributes` with **inline defaults** inside each method — do not hoist to module-level `DEFAULT_*` constants. Matches `fsg_internal_backend/src/infrastructure/repositories/*`. Example:

```javascript
static async findNotAcknowledgedByHospitalId (hospitalId, options = {}) {
  const { limit = 20, offset = 0, order = [['createdAt', 'DESC']], attributes = ['id', 'uuid', 'alertType', 'createdAt'] } = options
  return TestAlert.findAndCountAll({ where: { acknowledgedAt: null }, include: [/* ... */], order, limit, offset, attributes, raw: true, nest: true })
}
```

### AJV `removeAdditional: 'all'` mutates inner items of array args
The AJV instance (`src/libs/ajv.js`, both backends) uses `removeAdditional: 'all'`. When a service's `constraints` schema declares an array of objects with `items: { type: 'object', properties: {...} }`, AJV **strips every undeclared property from each item in place** during validation. Because the array is passed by reference, this mutates the **caller's** objects too — breaking downstream services that share the same array (e.g. `SubmitFormRequestService` handing `createdTests` through a service that strips fields).

- Does NOT fix it: `[...array]` (shallow — inner objects are the same refs); `additionalProperties: true` on items (`removeAdditional:'all'` overrides it); `items: { type: 'object' }` with no `properties` (strips everything).
- DOES fix it: change the service contract to accept the **primitive ids/uuids** it needs (caller extracts them first — primitives can't carry mutating references):

```javascript
const rejectedTestIds = pickRejectedTestIds(createdTests)
await CreateRejectedAlertsForTestsService.run({ rejectedTestIds }, this.context)
// child schema: { rejectedTestIds: { type: 'array', items: { type: 'number' } } }
```

If the full object is genuinely needed: deep-clone before passing, or drop the inner-items schema and trust the caller.

### Migrations (concrete)
Written with Sequelize `queryInterface`. FK columns: type + nullability only (no `references`, no `onUpdate/onDelete: 'CASCADE'`). No FK-lookup indexes. Unique invariants via `uniqueKeys` on `createTable` or `addConstraint`. For an already-migrated table, edit its migration for fresh deploys **and** provide the `ALTER TABLE …` for syncing existing DBs; new tables get a new migration file.

### AJV validation placement
Only services invoked by other services (via `.run()`) need their own `constraints` schema; controller-invoked services rely on route validation middleware.

---

**Remember**: simple, readable, secure, performant. If you need a comment to explain code, refactor the code instead.
