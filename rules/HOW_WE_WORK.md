# How We Work — FSG Agent Workflow

Working agreement for how Claude Code should operate on the FSG projects (2 backends, 2 frontends). Goal: **high quality, safe changes, and a main conversation that stays lean** so we can work long sessions without losing context.

## Golden rule (applies to everything)
I do my part; **you test all the changes at the end**. I **commit only when you explicitly tell me to** — otherwise never commit, even after you've approved the code.

## Project boundaries (frontend vs backend sessions)
- A session is scoped to **one side**, and may edit **both projects on that side**:
  - **Frontend session** → may edit both frontends (`fsg_front`, `fsg_internal_frontend`); the two backends are **read-only**.
  - **Backend session** → may edit both backends (`fsg_client_backend`, `fsg_internal_backend`); the two frontends are **read-only**.
- Read the read-only side freely for context/cross-referencing, but **never change its code**.
- If a task turns out to need a change on the other side, **stop and flag it** for a session scoped to that side — don't cross the boundary.

## Models & roles
- **Main agent: Opus** — holds our shared context; does reasoning, logic, orchestration, integration, verification. **Start every session on Opus.**
- **Sonnet** — dispatched for bulk/precise **reading & searching**, and for well-scoped structural work.
- **Fable** — strongest at **CSS / style / design / layout**; it's **token-heavy, so reserve it for style/design jobs (including style/CSS debugging)**, not general work.

## Backend (logic-heavy → slower but safe, highest quality)
- **Coding is done inline by the main (Opus) agent**, not delegated — so every change has full context and maximum fidelity.
- **Nothing lands without approval.** I propose/apply changes and you approve each one (permission mode set to ask on edits, or I pause). Backend data mistakes are hard to undo, so we favour safe + reviewed over fast.
- **Reading/searching is delegated** to Sonnet agents to keep the main context lean. Their briefs require **exact excerpts with `file:line`, not loose summaries**, to avoid sloppy-read data loss. For anything an edit depends on, the main agent **re-reads the exact lines** before editing.
- Agents follow **`BACKEND_RULES.md`** — give each agent the relevant group for its task (not the whole file).

## Frontend (quality-focused)
- **Logic → Opus (main), inline** — component/business logic is written by the main agent, same as backend.
- **CSS / style / design / layout → Fable** (dispatched via `SendMessage`, resumed to keep its context), including **style/CSS debugging**. The main agent **diagnoses, briefs, and compares renders against the mockups** (subagents can't see images); Fable edits. Fable is token-heavy — use it for these visual jobs, not for logic.
- **Reading/searching → Sonnet** (same as backend).
- Frontend work follows **`CLAUDE_FRONTEND_RULES.md`** — give each agent the relevant section for its task (not the whole file), same as backend agents get a `BACKEND_RULES.md` group.

## Frontend end-of-task review (Fable)
- When **all coding for the task is finished** (frontend sessions), before we wrap up, dispatch a **Fable subagent (latest version)** to review the complete frontend diff and clean it up against **`CLAUDE_FRONTEND_RULES.md`**.
- Fable checks for **rule compliance** and for **no extra, redundant, unused, or speculative code** (CSS, styles, components, props) — and fixes what it finds.
- The main agent briefs Fable with the relevant `CLAUDE_FRONTEND_RULES.md` section(s) and the list of changed files; Fable reports what it cleaned.
- Run this **once at the end** — it's the frontend counterpart to end-of-task verification, not a per-file step.

## Design options from Fable (before building a new screen)

For any new screen or non-trivial component, Fable produces options to choose from
**before** implementation, as a **standalone local `.html` file** — never a published
artifact. Deliver the path plus an `xdg-open` command.

- **3–4 structurally distinct options**, not colour variations: a card list vs a
  table vs a matrix vs a stepper. If two options differ only in styling, that is one
  option.
- **Mock realistic volume and mixed state.** A three-row happy path hides the
  problems. Show the list long enough to scroll, and put a failure and an in-progress
  item *below the fold* — that is how the need for a sticky summary was found.
- **Make it interactive** where behaviour is the thing being judged; clicking beats
  describing.
- **Brief Fable with verified facts**, not assumptions — see the backend section
  above. A mock built on a wrong field becomes a spec built on a wrong field.
- Iterate by resuming the same Fable agent so it keeps its context; state scope
  *removals* explicitly, since a dropped feature must be cleaned out of every mock.

## Verifying frontend work (there is no test framework)

`fsg_internal_frontend` has no test runner — `package.json` has only `dev`, `build`,
`lint`, `preview`. That is not a reason to skip verification, it changes its shape:

- **Pure modules** (transformers, validators, formatters) — bundle and assert in
  node. Verified working:
  ```bash
  SCRATCH=/tmp/fsg-harness && mkdir -p $SCRATCH
  ln -sfn "$PWD/node_modules" $SCRATCH/node_modules   # the harness imports dayjs etc.
  npx esbuild src/utils/<module>.ts --bundle --format=esm --platform=node \
    --outfile=$SCRATCH/m.mjs --log-level=error
  node $SCRATCH/check.mjs
  ```
  Harness files live in the scratch dir, **never** in the repo.
- **For a two-way mapping, assert the round trip** — `toFormValues(toApi(x)) === x`
  for every mapped key. That single assertion is what catches drift, and it is why
  the mapping belongs in one pure module (`CLAUDE_FRONTEND_RULES.md` §8).
- **Everything else** — `npx tsc --noEmit -p tsconfig.app.json` and `npx vite build`,
  then the user's browser pass. Neither proves behaviour; say so plainly rather than
  claiming a change is "tested".
- **Verify claims against the source before repeating them.** Several confident
  statements this session were wrong until checked — a control height, a row height,
  a call site missed by a case-sensitive grep. Read the line, then speak.

## Check the backend before designing against it

Before designing a screen around data, read the backend that produces it — the
service, the response schema, and the route's AJV schema. Three things this caught
that would otherwise have shipped:

- a mock gave every test its own Lab ID; `labId` is a property of the **form request**,
  so all tests in a registration share one, and the design had to change;
- an update payload type advertised four fields the endpoint silently strips;
- the label queue runs **one job per test at concurrency 1**, which invalidated the
  obvious "probe one, then fetch the rest" retry strategy.

A response field also has to survive the route's response schema — AJV strips what
is not declared, so confirm the field is in the schema, not merely in the service's
return.

## Teaching the frontend (user is a backend dev)
- When I introduce or use a **frontend concept the user may not know** (antd props, React hooks,
  router patterns, CSS behaviours, etc.), give a **short explanation**: what it is, what it does,
  and why we're using it here. Right-sized — not a wall of text, not a one-liner.
- **Don't re-explain** something already covered — a tiny mention ("same `sorter`/`onChange` pattern
  as before") is enough.
- Goal: help the user understand how the pieces fit together, since they own the backend and are
  learning the front side. Suggest faster-learning ideas when I spot them.

## Context discipline
- Delegate heavy **reads/search/exploration** to subagents — their large tool output stays in *their* context; only concise reports return here.
- **Fan out only for genuinely independent tasks** (that's where speed comes from). A single sequential subagent is *not* faster than inline — its win is context, not speed.
- Keep the main agent for reasoning, integration, and verification.

## codebase-memory MCP
- **Prefer the graph tools** (`search_graph`, `trace_path`, `get_architecture`, `query_graph`) over grep/Read for **structural questions** ("what calls this / what breaks if I change it / architecture overview") — they answer from the index at a fraction of the tokens. Fall back to grep/Read for literal text or when graph coverage is insufficient.
- It **cannot auto-update** (no write permission → it can't self-index). **Before every codebase-memory read/navigation** (`search_graph`, `query_graph`, `trace_path`, `get_code_snippet`, `search_code`, `get_architecture`), first refresh the index (`detect_changes` / `index_repository`).
- Any dispatched agent that uses codebase-memory must refresh the index first too.

## Commits
- **Never auto-commit** (see Golden rule — you test first, then explicitly tell me to commit). Propose grouped commits (groups + messages + files); you give the go-ahead, or you commit yourself.
- Commit in **logical groups**, one concern per commit — not one big commit.
- Branch prefix `feature/<name>`; Conventional Commit subjects (`feat(scope): …`); **no `Co-Authored-By` trailer**.

## Approval & interruption
- You accept/approve changes and interrupt the moment something looks wrong.
- Inline work gives per-step visibility; delegated work returns as a batch you review and correct.
