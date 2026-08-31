# Repository Guidelines

## SDKWORK Soul

Read `../sdkwork-specs/SOUL.md` before executing tasks in this root. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Use `../sdkwork-specs/README.md` as the task router and `../sdkwork-specs/AGENTS_SPEC.md` for this entrypoint. Do not copy global standards into this repository. If the relative specs path does not resolve, stop and report the broken workspace layout.

## Application Identity

Read `apps/sdkwork-promotion-pc/sdkwork.app.config.json` when changing PC application behavior, runtime config, SDK wiring, release metadata, packaging, app-owned capabilities, or deployment metadata. Stop if the manifest is missing, empty, or ambiguous.

- Domain: `commerce`
- Capability: `promotion`
- Table prefix: `commerce_`
- App API prefix: `/app/v3/api/promotions`
- Backend API prefix: `/backend/v3/api/promotions`
- PC application root: `apps/sdkwork-promotion-pc/`

## Local Dictionary Structure

- `AGENTS.md`: repository execution rules and standards router.
- `.sdkwork/`: local skills, plugins, manifests, and AI workspace metadata.
- `apps/sdkwork-promotion-pc/`: PC React application root.
- `apps/sdkwork-promotion-common/packages/`: shared TypeScript contracts and service normalization.
- `specs/`: repository-wide machine contracts.
- `apis/`: authored Promotion API contracts.
- `sdks/`: Promotion SDK families and generator-owned artifacts.
- `crates/`: Rust routes, services, repositories, hosts, and gateways.
- `database/`: Promotion database lifecycle assets.
- `etc/`: deployable source configuration and environment profiles.
- `deployments/`, `scripts/`, `tools/`, `docs/`, `tests/`: deployment, automation, documentation, and verification assets.
- `package.json`, `Cargo.toml`, `pnpm-workspace.yaml`: native build and dependency authorities.

Documentation Canon: [`docs/product/prd/PRD.md`](docs/product/prd/PRD.md) and [`docs/architecture/tech/TECH_ARCHITECTURE.md`](docs/architecture/tech/TECH_ARCHITECTURE.md).

## Spec Resolution Order

Use dynamic progressive loading before reading implementation files. Resolve the repository or application root and task boundary first. Read the applicable app manifest and nearest module `specs/component.spec.json`, then use the matching row in `../sdkwork-specs/README.md` to load only task-specific global specs before implementation files. Expand progressively only when evidence crosses another boundary. Language-specific specs are on-demand only.

## Required Specs By Task Type

- Agent/workflow changes: `../sdkwork-specs/AGENTS_SPEC.md` and `../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`.
- TypeScript or Node changes: `../sdkwork-specs/CODE_STYLE_SPEC.md`, `../sdkwork-specs/NAMING_SPEC.md`, and `../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md`.
- React UI changes: `../sdkwork-specs/FRONTEND_CODE_SPEC.md`, `../sdkwork-specs/FRONTEND_SPEC.md`, `../sdkwork-specs/UI_ARCHITECTURE_SPEC.md`, and `../sdkwork-specs/APP_PC_REACT_UI_SPEC.md`.
- Rust/runtime changes: `../sdkwork-specs/RUST_CODE_SPEC.md`, `../sdkwork-specs/WEB_FRAMEWORK_SPEC.md`, `../sdkwork-specs/WEB_BACKEND_SPEC.md`, and the applicable runtime/database spec.
- SDK generation or consumption: `../sdkwork-specs/SDK_SPEC.md`, `../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md`, `../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md`, and `../sdkwork-specs/API_SPEC.md`.
- API contract changes: `../sdkwork-specs/API_SPEC.md`; list/search work also loads `../sdkwork-specs/PAGINATION_SPEC.md`.
- Database changes: `../sdkwork-specs/DATABASE_FRAMEWORK_SPEC.md`, `../sdkwork-specs/SUBJECT_ID_SPEC.md`, and applicable database specs.
- Package/release changes: `../sdkwork-specs/PNPM_SCRIPT_SPEC.md`, `../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`, `../sdkwork-specs/RELEASE_SPEC.md`, and `../sdkwork-specs/SUPPLY_CHAIN_SECURITY_SPEC.md`.

## Code Style Rules

Use native workspace helpers and `@sdkwork/utils`; do not create local duplicates. Generated output under `sdks/**/generated/**` is generator-owned. Keep app/backend API boundaries separate, use `sdkwork-web-framework` for Rust HTTP response mapping, and do not bypass generated/composed SDKs with raw HTTP or manual auth headers.

## Int64 Wire Contract (API_SPEC §13.6)

- OpenAPI `int64` fields and parameters `MUST` be `type: string`, `format: int64`,
  a decimal `pattern` such as `^-?[0-9]+$`, and `x-sdkwork-int64-string: true`.
  `type: integer, format: int64` is a contract violation: generated TypeScript
  SDKs then emit `number`, and browsers silently round ids past
  `Number.MAX_SAFE_INTEGER` (2^53), replaying wrong ids into lookups.
- Rust response DTOs `MUST` serialize `i64` wire fields with
  `#[serde(with = "sdkwork_utils_rust::serde_int64")]` (or `::option`); request
  boundaries parse inbound strings with the same helper.
- Generated TypeScript SDKs keep `int64` as `string`; frontend code `MUST NOT`
  convert ids/snowflake ids/sequence ids to `number` for storage, comparison,
  or submission.
- Verification: `node <sdkwork-specs>/tools/check-api-operation-patterns.mjs --workspace .`

## Build, Test, And Verification

Use repository scripts as command authority. Run narrow checks before `pnpm verify`. Relevant checks include:

```bash
node ../sdkwork-specs/tools/check-agent-workflow-standard.mjs --root .
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
node ../sdkwork-specs/tools/check-api-operation-patterns.mjs --workspace .
node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
pnpm check
pnpm verify
```

## Agent Execution Rules

Inspect dirty worktree changes before editing and preserve unrelated user work. Use exact contracts before inference, never hand-edit generated SDK output, and stop when an owner contract, SDK method, app manifest, or required relative spec is ambiguous or missing. Record command evidence before claiming completion.

## Task-Specific Standards Routing

- App SDK consumer work loads `APP_SDK_INTEGRATION_SPEC.md`, `SDK_SPEC.md`, and `SDK_WORKSPACE_GENERATION_SPEC.md`, then runs `check-app-sdk-consumer-imports.mjs`.
- HTTP API response work loads `API_SPEC.md`, then runs `check-api-operation-patterns.mjs` and `check-api-response-envelope.mjs`.
- List and search work loads `PAGINATION_SPEC.md` and `API_SPEC.md`, then runs `check-pagination.mjs`.

## Human Review Rules

Request human review before breaking public contracts, changing application identity, changing security/auth behavior, changing database migrations, changing generated SDK ownership, using production credentials, changing release/deployment governance, or performing destructive cleanup.

<!-- SDKWORK-NAMING-STANDARD: v1 -->
## Rust Naming And Dependency Declaration

Authority: `../sdkwork-specs/NAMING_SPEC.md` section 3.1 and section 3.2.

Two identifier planes exist in every Rust crate and they MUST NOT be mixed: the package plane
(Cargo, filesystem, lock file) uses kebab-case, and the crate plane (lib target, modules, source
imports) uses snake_case.

- `[package].name`, the crate directory, `[features]` keys, and `[[bin]].name` use kebab-case.
- `[lib].name`, module files, module directories, and Rust imports use snake_case.
- A crate whose `[package].name` contains a hyphen SHOULD declare `[lib].name` explicitly
  (default: package name with every `-` replaced by `_`). A shorter lib name is allowed only
  when declared explicitly and used consistently by every consumer.
- Cargo dependency keys, `[workspace.dependencies]` keys, and `Cargo.lock` entries use the
  dependency package name. Use `package = "..."` when an alias is required.
- Every external crate referenced by `src/` MUST be declared in that crate's `[dependencies]`.
  Test-only crates belong in `[dev-dependencies]`; `build.rs` crates belong in
  `[build-dependencies]`.
- Never delete a dependency line, and never demote one from `[dependencies]` to
  `[dev-dependencies]`, while `src/` still imports it. Verify manifest cleanups with the
  command below before committing them.
- Regenerate and commit `Cargo.lock` in the same change as any dependency table edit.

Verification:

```bash
node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root .
```
<!-- /SDKWORK-NAMING-STANDARD: v1 -->

<!-- SDKWORK-RUST-CODE-STANDARD: v1 -->
## Rust Code Standard

Authority: `../sdkwork-specs/RUST_CODE_SPEC.md` (v2, industry-best baseline); package/crate
naming and dependency declaration are normative in `../sdkwork-specs/NAMING_SPEC.md` section 3.1
and 3.2.

- Crates are responsibility-shaped: service, repository-sqlx, routes, service-host, native-host,
  worker, assembly, gateway. No generic `core`/`common`/`backend`/`runtime` suffixes.
- Errors are typed enums (`thiserror`) implementing `std::error::Error` with a `source` chain.
  `anyhow` only at binary/CLI/test boundaries, never in lib `[dependencies]`.
- No `unsafe` without a `// SAFETY:` comment; crates default to `unsafe_code = "forbid"`.
  No `unwrap`/`expect`/`panic!`/`todo!`/`dbg!` in library code reachable from public API.
- No lock guard held across `.await`; every external await has a timeout; spawned tasks are
  awaited/detached with a documented owner; retries are bounded, jittered, and idempotent.
- Public API is minimal, documented, `#[must_use]` where applicable, and semver-clean. Leaking
  framework types (`sqlx::Row`, axum extractors) through public signatures is forbidden.
- Workspace root declares `[workspace.package]` (edition, rust-version) and `[workspace.lints]`
  (RUST_CODE_SPEC.md section 13 baseline); every member inherits both with
  `edition.workspace = true` and `[lints] workspace = true`.

Verification:

```bash
node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root .
node ../sdkwork-specs/tools/check-rust-manifest-standard.mjs --root .
# when service/repository/route/gateway dependencies change:
node ../sdkwork-specs/tools/check-rust-backend-composition.mjs --root .
```
<!-- /SDKWORK-RUST-CODE-STANDARD: v1 -->

<!-- SDKWORK-TYPESCRIPT-CODE-STANDARD: v1 -->
## TypeScript Code Standard

Authority: `../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md` (v2, industry-best baseline).

- `tsconfig` runs `strict: true` and the strict family; public APIs are typed and `any`-free.
  `import type` is required for type-only imports (`verbatimModuleSyntax`).
- Errors are typed at package/service boundaries; no empty catches, no swallowed promise
  rejections, no bare `throw new Error('...')` for business failures.
- Async: every promise is settled; external awaits have timeouts; `AbortSignal` accepted for
  cancellable work; bounded concurrency; no unbounded `Promise.all`.
- Public API is minimal, JSDoc-documented, `@deprecated` where applicable, and semver-clean.
- Discriminated unions model closed variant sets; no `as`/`@ts-ignore` bypasses without a guard.
- Node/build runners verify build-critical sources and self-heal from git (CODE_STYLE_SPEC §7);
  `pnpm clean` never deletes git-tracked build-critical files.

Verification:

```bash
pnpm typecheck && pnpm test && pnpm lint
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
```
<!-- /SDKWORK-TYPESCRIPT-CODE-STANDARD: v1 -->

<!-- SDKWORK-FRONTEND-CODE-STANDARD: v1 -->
## Frontend Code Standard

Authority: `../sdkwork-specs/FRONTEND_CODE_SPEC.md` (v2); language rules follow
`../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md` (React/TS) or `../sdkwork-specs/DART_CODE_SPEC.md` (Flutter).

- UI -> service -> injected SDK flow is preserved; components never construct SDK clients or
  assemble raw HTTP/auth headers.
- React: hooks rules clean (`react-hooks`), `useEffect` with full deps and cleanup, stable
  list keys, error boundaries at route/page level, derived state during render (not in effects).
- State: server state behind services/query layer; client state local or minimal typed store;
  no duplication of server state in client stores.
- Accessibility: accessible names, keyboard behavior, visible focus, color is never the only
  signal; error states announced.
- i18n for all user-facing copy in reusable/user-facing packages (I18N_SPEC §6.1).
- PC/H5 `outDir` uses `dist/{standalone,cloud}/{dev,test,staging,prod}`.

Verification:

```bash
pnpm typecheck && pnpm test && pnpm lint
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-browser-dist-layout.mjs --root .   # PC/H5 apps
```
<!-- /SDKWORK-FRONTEND-CODE-STANDARD: v1 -->

<!-- SDKWORK-PNPM-WORKSPACE-STANDARD: v1 -->
## pnpm Workspace Dependency And Package Import

Authority: `../sdkwork-specs/PNPM_WORKSPACE_DEPENDENCY_SPEC.md` (companion to
`../sdkwork-specs/DEPENDENCY_MANAGEMENT_SPEC.md`).

Sibling SDKWork repositories are consumed through a dual-track model that MUST stay consistent:

- **Local development** (`pnpm dev`, `pnpm build`): pnpm workspace protocol. Each sibling
  package is declared ONCE in this repository root `pnpm-workspace.yaml` `packages:` as a
  `../sdkwork-*` relative path, and consumed with `workspace:*` in `package.json`. Never use
  `file:`/`link:`/git-URL specifiers for SDKWork sibling packages in any environment.
- **CI / release packaging**: git-repository dependency checkout. Every sibling referenced by the
  local workspace MUST have a matching `dependencies[]` entry in `sdkwork.workflow.json` so CI
  clones the sibling into the same `../sdkwork-*` relative layout (`GITHUB_WORKFLOW_SPEC.md`).
  `package.json` is never rewritten for CI.

Import rules for sibling SDKWork packages:

- Import by package name only: `import { X } from "@sdkwork/package-name"`. The specifier MUST
  equal the target package's `package.json` `name` exactly - no shortening, renaming, or alias.
- Forbidden: relative imports that cross a package boundary into another SDKWork repository or
  another workspace package's `src/` (for example `import ... from "../../sdkwork-appbase/.../src/..."`).
- Consume only the public `exports` surface of a package; never deep-import sibling `src/` internals.
- Every non-relative import in a workspace member MUST resolve to that member's own
  `dependencies`/`devDependencies`/`peerDependencies` (import closure).
- Vite aliases MUST NOT rename or redirect `@sdkwork/*` packages, MUST NOT be added to make a
  resolution error pass, and are allowed only for documented bootstrap/SDK-generation entrypoints.
- Fix a resolution failure by correcting the workspace declaration or the package `exports`,
  not by adding an alias.

Verification:

```bash
node ../sdkwork-specs/tools/verify-repo.mjs --root .
node ../sdkwork-specs/tools/check-workspace-member-protocol.mjs --root .
node ../sdkwork-specs/tools/check-dependency-list-completeness.mjs --target <repo-name>
```
<!-- /SDKWORK-PNPM-WORKSPACE-STANDARD: v1 -->
