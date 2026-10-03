# Application Guidelines

## SDKWORK Soul

Read `../../../sdkwork-specs/SOUL.md` before executing tasks in this application root. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Use `../../../sdkwork-specs/README.md` as the task router and `../../../sdkwork-specs/AGENTS_SPEC.md` for this entrypoint. Do not copy global standards into this application. If the relative specs path does not resolve, stop and report the broken workspace layout.

## Application Identity

Read `sdkwork.app.config.json` when changing application behavior, runtime config, SDK wiring, release metadata, packaging, app-owned capabilities, or deployment metadata. Stop if the manifest is missing, empty, or ambiguous.

- Application root: `apps/sdkwork-promotion-pc/`
- Domain: `commerce`
- Capability: `promotion`
- Architecture: `pc-react`

## Local Dictionary Structure

- `AGENTS.md`: application execution rules and standards router.
- `.sdkwork/`: application-local skills, plugins, and workspace metadata.
- `sdkwork.app.config.json`: application identity and release manifest.
- `packages/`: React packages for promotion UI surfaces.
- `specs/`: application-level machine contracts.
- `src/`, `tests/`, `vite.config.ts`, `tsconfig.json`: shell source, tests, and build configuration.

## Spec Resolution Order

Use dynamic progressive loading before reading implementation files. Read this file, the app manifest, and the nearest module `specs/component.spec.json` first. Then use `../../../sdkwork-specs/README.md` to load only the global specs required by the task. Repository-wide work also loads `../../AGENTS.md`. Language-specific specs are on-demand only.

## Required Specs By Task Type

- Agent/workflow changes: `../../../sdkwork-specs/AGENTS_SPEC.md` and `../../../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`.
- TypeScript changes: `../../../sdkwork-specs/CODE_STYLE_SPEC.md`, `../../../sdkwork-specs/NAMING_SPEC.md`, and `../../../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md`.
- React UI changes: `../../../sdkwork-specs/FRONTEND_CODE_SPEC.md`, `../../../sdkwork-specs/FRONTEND_SPEC.md`, `../../../sdkwork-specs/UI_ARCHITECTURE_SPEC.md`, and `../../../sdkwork-specs/APP_PC_REACT_UI_SPEC.md`.
- SDK consumption: `../../../sdkwork-specs/SDK_SPEC.md`, `../../../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md`, and `../../../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md`.
- API integration: `../../../sdkwork-specs/API_SPEC.md`; list/search work also loads `../../../sdkwork-specs/PAGINATION_SPEC.md`.
- App identity/release: `../../../sdkwork-specs/APP_MANIFEST_SPEC.md`, `../../../sdkwork-specs/CONFIG_SPEC.md`, and `../../../sdkwork-specs/DEPLOYMENT_SPEC.md`.
- Package scripts: `../../../sdkwork-specs/PNPM_SCRIPT_SPEC.md`.
- Packaging workflows: `../../../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`, `../../../sdkwork-specs/PNPM_SCRIPT_SPEC.md`, and `../../../sdkwork-specs/DEPLOYMENT_SPEC.md`.

## Code Style Rules

React packages use `@sdkwork/promotion-pc-*` naming. Use `@sdkwork/utils` and public package exports. Consume SDKWork APIs through generated/composed SDK clients and injected service ports; do not create raw HTTP, manual auth headers, private source imports, legacy envelopes, or `requestId` wire fields.

## Build, Test, And Verification

Run the narrowest package typecheck or test first, followed by repository checks when boundaries change:

```bash
node ../../../sdkwork-specs/tools/check-agent-workflow-standard.mjs --root ../../..
node ../../../sdkwork-specs/tools/check-application-layering.mjs --root ../../..
node ../../../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace ../../..
node ../../../sdkwork-specs/tools/check-api-operation-patterns.mjs --workspace ../../..
node ../../../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace ../../..
node ../../../sdkwork-specs/tools/check-pagination.mjs --workspace ../../..
pnpm check
pnpm verify
```

## Agent Execution Rules

Inspect dirty worktree changes before editing and preserve unrelated user work. Never hand-edit generated SDK output or bypass generated/composed SDKs. Stop when the app manifest, owner contract, SDK method, or required relative spec is missing or ambiguous. Record command evidence before claiming completion.

## Task-Specific Standards Routing

- App SDK consumer work loads `APP_SDK_INTEGRATION_SPEC.md`, `SDK_SPEC.md`, and `SDK_WORKSPACE_GENERATION_SPEC.md`, then runs `check-app-sdk-consumer-imports.mjs`.
- HTTP API response work loads `API_SPEC.md`, then runs `check-api-operation-patterns.mjs` and `check-api-response-envelope.mjs`.
- List and search work loads `PAGINATION_SPEC.md` and `API_SPEC.md`, then runs `check-pagination.mjs`.

## HTTP API Response Envelope

All L2+ SDKWork-owned custom HTTP contracts, including `app-api`, `backend-api`, and SDKWork-owned business `open-api`, `MUST` follow `API_SPEC.md` section 4.5, section 14, and section 15:

- **Default classification:** omitted `x-sdkwork-wire-protocol` means SDKWork-owned custom API (`sdkwork-v3`); only operation-level `x-sdkwork-wire-protocol: external` plus `x-sdkwork-external-protocol-id` identifies a third-party compatibility `open-api` operation.
- **Input:** typed request bodies, section 14.1 list/search/command input, `SdkWorkListQuery`, and `q` for free-text search.
- **Success output:** `SdkWorkApiResponse` with `{ "code": 0, "data": <payload>, "traceId": "<server-uuid>" }`.
- **Error output:** HTTP 4xx/5xx `application/problem+json` (`ProblemDetail`) with numeric `code` and `traceId`; SDKWork-owned errors may include `i18nKey` and `locale` presentation metadata.
- Success `code` is numeric `int32`; HTTP 2xx JSON bodies `MUST` use `0` only. REST semantics remain on HTTP status (`201`, `202`, etc.).
- Platform error codes are numeric non-zero values per section 15.3 (`40001`, `40101`, `40401`, …).
- Single resource: `data.item`
- Lists: `data.items` + `data.pageInfo` (`PageInfo.mode` is `offset` or `cursor`)
- Commands: `data.accepted` plus optional `resourceId` / `status`
- Async accept (`202`): `data.operationId`, `data.status`, optional `pollUrl`
- Operation patterns: retrieve/list/search/create/update/delete/command/async/bulk semantics follow `API_SPEC.md` section 15.4; create uses `201`, delete uses `204` with no JSON body, and `PUT`/`PATCH` use SDK action `update`.

Vendor compatibility `open-api` routes that mirror upstream tool or provider wire (for example OpenAI `/v1/*`, Anthropic/Claude `/anthropic/v1/*`, Google/Gemini `/google/v1beta/*`, Claude Code, or Codex) `MAY` opt out only when every exempt operation declares operation-level `x-sdkwork-wire-protocol: external` and `x-sdkwork-external-protocol-id` per `API_SPEC.md` section 4.5.2. SDKWork-owned business `open-api` operations `MUST NOT` opt out. Mixed OpenAPI documents are validated per operation; one external operation never exempts SDKWork-owned operations in the same document.

Errors `MUST` use HTTP 4xx/5xx with `application/problem+json` (`ProblemDetail`) including required numeric `code` and `traceId`. Optional `i18nKey` and `locale` are display metadata only. Business failures `MUST NOT` use HTTP 2xx with non-zero `code`, string wire codes, `success`, or human `message`.

Forbidden legacy envelopes and fields: `PlusApiResult`, `AppbaseApiResult`, `StoreApiResult`, `SdkWorkResponse`, per-domain `*ApiResult`, wire field `requestId`, bare domain DTOs at the HTTP root, and top-level `{ items, pageInfo, traceId }` without `data`.

Handlers `MUST` serialize success and map errors through `sdkwork-web-framework` response mapping. Generated HTTP SDKs (`--standard-profile sdkwork-v3`) unwrap `data` by default and expose typed numeric `ProblemDetail.code` / `traceId` and returned localization metadata on errors; use `.raw` when the full envelope is required.

Before completing API contract, SDK generation, or frontend service work, run:

```bash
node <sdkwork-specs>/tools/check-api-operation-patterns.mjs --workspace <workspace-root>
node <sdkwork-specs>/tools/check-api-response-envelope.mjs --workspace <workspace-root>
```

Authority: `sdkwork-specs/API_SPEC.md` section 4.5 and sections 14–16, `SDK_SPEC.md` section 4.2, `FRONTEND_SPEC.md`, `MIGRATION_SPEC.md` section 4.2.

## Human Review Rules

Request human review before breaking public API/SDK behavior, changing application identity, changing runtime or release metadata, changing security/auth behavior, changing database migrations, changing generated SDK ownership, or performing destructive cleanup.
