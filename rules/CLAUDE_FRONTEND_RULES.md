# Claude Frontend Rules

Applies to both frontend projects: **`fsg_front`** (genome-portal) and
**`fsg_internal_frontend`** (fsg-crm).

Shared stack: **Vite + React 19 + TypeScript + Ant Design (antd v6) +
react-router-dom v7**.

> Living document. Sections 1–2, 10 and 11 are frontend-native;
> sections 3–8 mirror the universal principles in **`BACKEND_RULES.md`**, adapted
> to React/TS — the two docs should stay consistent. Every rule earns its place by
> solving a real problem we hit — no cargo-culting.

## 1. Clean architecture

Keep responsibilities in separate layers. A change in one concern should not
force edits across unrelated files.

- **`pages/`** — one folder per screen. Composes components; owns page-level
  wiring (which data to load, which components to show). Thin.
- **`components/`** — reusable UI pieces. Presentational by default: take props,
  render UI, emit events. No direct API calls.
- **`api/`** — all backend communication (client + endpoints). The only layer
  that knows about HTTP, URLs, and response shapes.
- **`utils/`** — pure helpers (formatters, validators, transformers). No React,
  no side effects — same input always gives same output.
- **`contexts/` + hooks** — shared state and reusable stateful logic.
- **`types/`** — shared TypeScript types/models. One source of truth.
- **`config/`, `styles/`** — constants, routes, design tokens, theme.

Rule of thumb: **data flows `api → hooks/context → page → components`; events
flow back up.** Components never reach past their layer.

### A hook is the use case

The backend's layers map onto React under different names. A **custom hook is the
service**: it holds state, calls the `api/` layer and exposes actions. It returns
data, never markup.

| Backend | Frontend | Its one job |
|---|---|---|
| Repository | `api/` module | Talk HTTP. URLs and response shapes, nothing else. |
| Service / use case | custom hook | Orchestrate. State, effects, API calls, navigation. |
| Mapper / DTO transform | pure function in `utils/` | Shape A → shape B. No React, no I/O. |
| Controller | the page | Wire the use case to the view. Thin. |
| — | component | Render. Props in, events out. |

**Where does this line of code go?** Four questions, in order:

1. Does it touch the network? → `api/`
2. Is it a pure input→output transform? → `utils/`. If it can't be written without
   React, it isn't pure — look again.
3. Does it hold state or run over time? → hook
4. Otherwise → component

A component returning JSX is not a smell; that is its contract. The smell is a
component that *also* fetches, maps, validates and navigates.

### Pass the difference in; don't branch on the caller

When one region of a shared component differs per screen, take it as a `ReactNode`
prop and let the parent supply it. The component then never asks *who* is using it.

```tsx
patientSection: React.ReactNode;   // a search widget when creating, a summary when editing
```

This is the mechanism that makes "no mode flags" (§2) achievable rather than
aspirational: without a slot, the only way to vary that region is a flag naming the
screen, and every future difference gets bolted onto the same flag.

### View state belongs to the view

State that only decides what is on screen — a modal being open, an expanded row, a
cycling pointer — lives in the component. The hook owns the use case: what was
loaded, what was chosen, what will be submitted.

Test: if the state disappeared on remount and nothing was lost, it is view state.
`testModalOpen` in `RegistrationFields` qualifies; the selected tests do not.

## 2. Clean coding

- **One responsibility per unit.** A component or function does one thing. If it
  needs "and" to describe it, split it.
- **No business logic buried in JSX.** Compute above the `return`; keep the
  markup readable.
- **Names say intent.** `patientsAwaitingReview`, not `data2`.
- **Types, not `any`.** Reuse the models in `types/`; extend them, don't redeclare.
- **DRY, within reason.** Extract a helper/component on the *second* real
  duplication, not speculatively.
- **Early returns** over deep nesting; **`async/await`** over nested `.then()`.
- **Imports stay on one line.** Don't break a named-import list across multiple
  lines, however long it gets — one module, one line. A long line is easier to
  scan and to diff than a block that reflows every time a name is added.
- **A controlled collection takes `value` + `onChange`, and derives its mutations.**
  Pass `tests` and `onTestsChange`; build add, remove and update *inside* from those
  two. Three more callback props would put the same list's rules in two places, and
  the parent stops being the single source of truth.
  ```tsx
  const handleRemoveTest = (testId: string) => onTestsChange(tests.filter((test) => test.id !== testId));
  ```
- **A fragment repeated three times becomes a named component**, declared at module
  scope in the same file — not exported, not moved to its own folder. `LockOverlay`
  appears four times in `RegistrationFields`; as inline JSX it was four chances to
  edit three of them.
- **Map, don't chain ternaries.** A value that selects one of a fixed set gets a
  `Record` at module scope, so adding a case is one line and the compiler checks
  exhaustiveness:
  ```tsx
  const STATUS_CLASS: Record<LabelStatus, string> = { preparing: styles.segPreparing, ready: styles.segReady, ... };
  ```
- **No mode flags in a shared component.** A prop says what the user *may do*
  (`samplesReadOnly`, `filesRequired`), never *which screen this is* (`isEditMode`).
  A dozen props carrying real data is healthy; one named after a screen means two
  use cases are hiding inside one component and will drift apart. The mode belongs
  in the hook, which is where the two flows are already separate.
- **Small diffs, clear why.** Each change should be explainable in one sentence:
  what problem it solves and why this approach over the alternatives.

## 3. Inputs & validation

- **Normalize/trim inputs before any processing** — trim strings, lowercase
  emails, canonicalize identifiers — *before* validation, submit, or state
  updates, so every downstream step (and the API) sees the clean value.
- **Validate at the boundary.** Validate form input before calling the API
  (shape, required fields, and the business rules you can check client-side).
- **Client validation is for UX, not trust.** The server re-validates everything;
  never rely on the frontend for security or as the only guard.

## 4. Data fetching & state

- **Fetch only what a screen needs** — don't over-fetch or pull whole datasets to
  render one view. Push filtering/pagination to the server for large lists.
- **Avoid request waterfalls (the frontend N+1)** — batch/parallelize independent
  requests; never fire one-request-per-row in a loop.
- **Cache/reuse server data** (query cache/context) with sensible invalidation on
  mutation; don't refetch the same thing on every render.
- **Debounce expensive user-driven calls** (search-as-you-type); the render path
  stays cheap.
- **Keep state where it's used;** lift only when shared. One source of truth per
  piece of data — no duplicated copies drifting out of sync.

## 5. Error handling

- **One consistent mechanism.** Every API call handles both success and failure;
  don't mix silent failures with thrown errors.
- **Never swallow errors** — no empty `catch`. Surface a user-facing message (and
  log to console for debugging); never leave the user on a frozen or blank UI.
- **User-facing messages stay generic and safe** — don't leak internals, stack
  traces, or which field failed for auth. Show what the user can act on.

## 6. Security

- **Prevent XSS** — let React escape by default; **avoid `dangerouslySetInnerHTML`**
  (sanitize if it's truly unavoidable). Never inject untrusted HTML.
- **Anything shipped to the browser is public.** Config from env
  (`import.meta.env` / `VITE_*`), never hard-coded — and **no real secrets in
  frontend code** (keys, tokens live server-side).
- **UI authorization is UX, not security.** Guard routes / hide actions by
  permission for a clean experience, but the server enforces every check (deny by
  default).
- **Don't log sensitive data** (tokens, PII) to the console.

## 7. Performance

- **Import only what you use** — no whole-library imports; prefer per-module /
  tree-shakeable imports (icons, utils, lodash-style helpers).
- **Avoid needless re-renders** — stable `key`s, memoize expensive
  computations/components where it's measured to matter, and don't create new
  objects/functions inline in hot paths.
- **Code-split heavy routes/components** (lazy-load) so the initial bundle stays
  small.
- **Keep slow work off the render path** — debounce/throttle user input; do async
  work in effects, not during render.

## 8. Types & contracts

- **A shape change is a full loop.** When an API response/DTO changes, update the
  **`api/` layer, the `types/` model, AND every consumer together** — a stale type
  hides a broken field at runtime.
- **Named constants for enums/statuses/routes** — no magic strings; reference the
  shared constant, and convert neighbouring literals when you touch them.
- **One owner per mapping, both directions side by side.** A form-shape ↔ API-shape
  conversion lives in exactly one pure module, with the write and the read of each
  value adjacent — two lookup tables that are visibly inverses, not two functions in
  two files. Every caller goes through it; no caller writes a mapped field itself.
  *Why:* sample registration mapped its clinical fields twice, once for create and
  once for edit. They drifted: create wrote `pregnancyType: "singleton"`, edit wrote
  `"single"`, and only `"singleton"` could be read back — so editing a record made
  the field unreadable to the screen that saved it. Dates drifted the other way,
  create sending UTC and storing every date a day early. Neither was caught by
  review; only a single owner makes them impossible.
- **The type must match what the endpoint accepts, not what you wish it accepted.**
  A type advertising fields the API strips is a lie the compiler will help you tell.
  If a cast is needed to satisfy a request type, the type is wrong — a double cast
  (`as unknown as X`) is never the fix.

## 9. Codebase navigation (MCP)

Structural code questions in these repos ("what calls this / what breaks if I
change it / architecture overview") go through the `codebase-memory` graph tools
rather than grep/Read, at a fraction of the tokens; fall back to grep/Read for
literal text (JSX copy, class names) or when graph coverage is insufficient.
**It does not auto-update — refresh the index before every read** — see the
codebase-memory section in **`HOW_WE_WORK.md`** for the operating rule.

## 10. Readable by default

- **Measure contrast, don't eyeball it.** Body text clears **4.5:1** against its
  actual composited background; check the computed value rather than trusting a
  token name. *Why:* a helper line in the label popup shipped at `rgba(0,0,0,.25)`
  — **1.83:1**, under half the readable threshold — and a failure note sat at
  3.35:1, unreadable at exactly the moment it mattered. Both looked fine in review.
- **A faint grey is a decision, not a default.** Muting text says "you may skip
  this". Before muting, ask whether the reader can actually afford to skip it.
- **Different kinds of message get different weight.** Interaction help ("click a
  count to step through them") is read once and discarded. A safety net ("closing
  loses nothing — print these later from Test Tracker") is the sentence that stops
  someone panicking. Styling them identically guarantees the important one is
  skimmed past with the disposable one. Separate them, and put the reassurance
  where the worry happens.
- **Say what the control will do before it is clicked**, when clicking is costly or
  hard to undo. A label print button reads `Print vial 3 × 2 cm`, not `Print`,
  because the operator must pick the matching printer in a dialog the page cannot
  influence.

## 11. Self-review the diff — nothing redundant

Before calling any change done, re-read the **whole diff** and remove anything that
isn't actually used or doesn't earn its place. Every added line must be needed and
working; no speculative or leftover cruft.

Check for:

- **Unused CSS** — classes never referenced, properties with no effect, redundant
  overrides of antd/framework defaults. Verify every `styles.*` in the JSX resolves
  to a class that is actually applied, and every CSS class is referenced.
- **Speculative "polish"** — transitions, hover states, shadows, animations nobody
  asked for. Default styling to **structural + accessibility only**; add flourish
  only when requested or clearly warranted.
- **Dead code** — unused imports, props, state, handlers, or variables; commented-out
  blocks; leftover scaffolding from an earlier approach.
- **Duplication** — the same value/logic repeated where one source would do.

Rule of thumb: if you can delete it and nothing breaks or looks wrong, delete it.
