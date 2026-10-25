# Promotion Technical Architecture

Status: active
Owner: SDKWork maintainers
Updated: 2026-07-08
Specs: ARCHITECTURE_DECISION_SPEC.md, RUST_CODE_SPEC.md, API_SPEC.md, WEB_FRAMEWORK_SPEC.md, DATABASE_FRAMEWORK_SPEC.md, APP_PC_ARCHITECTURE_SPEC.md

## 1. Architecture Overview

`sdkwork-promotion` is a T1 capability repository in the commerce domain. It owns domain services, SQL repositories, HTTP route builders, SDK families, common TypeScript service contracts, PC application packages, and a standalone gateway with IAM middleware.

```text
sdkwork-routes-promotion-* -> build_*_router() without IAM
sdkwork-api-promotion-standalone-gateway -> sdkwork-web-framework IAM/request-context middleware
apps/sdkwork-promotion-pc -> PC React feature packages and shell
```

Migration status: complete for active promotion ownership. Release workflow governance remains a pre-launch gap until reviewed workflow files are added.

## 2. Technology Choices

- Rust domain services and SQLx repositories (`RUST_CODE_SPEC.md`).
- Axum HTTP routers integrated through `sdkwork-web-framework` (`WEB_FRAMEWORK_SPEC.md`).
- SQLx for PostgreSQL repository implementations (DATABASE_SPEC: authoritative-server persistence is PostgreSQL-only; SQLite is client-local).
- TypeScript React PC packages following SDKWork PC app package boundaries (`APP_PC_ARCHITECTURE_SPEC.md`).
- Sibling path dependencies from this repository's `Cargo.toml`; local Rust crates use `sdkwork_promotion_*` package aliases and canonical `sdkwork-<domain>-<capability>-*` crate names.

## 3. System Boundaries And Modules

| Layer | Owner | Notes |
| --- | --- | --- |
| Domain commands/queries | `sdkwork-commerce-promotion-service` | Business validation and ports |
| SQL repositories | `sdkwork-commerce-promotion-repository-sqlx` | Tenant-scoped PostgreSQL persistence (DATABASE_SPEC) |
| HTTP route builders | `sdkwork-routes-promotion-app-api`, `sdkwork-routes-promotion-backend-api` | `build_*_router` exports without IAM |
| IAM / gateway composition | `sdkwork-api-promotion-standalone-gateway` | IAM middleware at the standalone gateway |
| Gateway assembly | `sdkwork-api-promotion-assembly` | Deterministic route composition inputs |
| Common TypeScript packages | `apps/sdkwork-promotion-common/packages/` | Contracts, SDK ports, and service package family |
| PC app packages | `apps/sdkwork-promotion-pc/packages/` | Coupon, offer, pricing, points, core, and shell packages |
| OpenAPI / SDK authority | `sdkwork-promotion/sdks/` | Per-capability SDK families |

## 4. Directory And Package Layout

Standard capability workspace:

- `crates/sdkwork-commerce-promotion-service/`
- `crates/sdkwork-commerce-promotion-repository-sqlx/`
- `crates/sdkwork-routes-promotion-app-api/`
- `crates/sdkwork-routes-promotion-backend-api/`
- `crates/sdkwork-promotion-database-host/`
- `crates/sdkwork-promotion-service-host/`
- `crates/sdkwork-api-promotion-assembly/`
- `crates/sdkwork-api-promotion-standalone-gateway/`
- `apps/sdkwork-promotion-common/packages/`
- `apps/sdkwork-promotion-pc/`

## 5. API, SDK, And Data Ownership

- App API prefix: `/app/v3/api/promotions`
- Backend API prefix: `/backend/v3/api/coupons`
- Table prefix: `commerce_` for capability-owned tables (`DOMAIN_SPEC` domain `commerce`)
- Public SDK consumption: generated per-capability SDK families and composed facades; feature packages must not handcraft raw HTTP (`SDK_SPEC.md`, `APP_SDK_INTEGRATION_SPEC.md`)
- List/search APIs must use store-level pagination and `SdkWorkApiResponse.data.items` plus `data.pageInfo` (`PAGINATION_SPEC.md`)

## 6. Security, Privacy, And Observability

- Authentication and tenant context are applied at the standalone gateway through SDKWork web framework middleware; handlers consume typed request context.
- Write routes require idempotency and request-hash headers where applicable (`API_SPEC.md`, `SECURITY_SPEC.md`).
- Ledger, payment, and account mutations fail closed on validation or dependency errors.
- Error responses use `application/problem+json` with numeric `code` and `traceId`; success responses use `SdkWorkApiResponse`.
- Internal SQL, dependency, and provider errors must not leak to API schemas or frontend state.

## 7. Deployment And Runtime Topology

- Local development: `pnpm dev` for the PC application and `cargo test --workspace` for Rust crates.
- Standalone runtime: `sdkwork-api-promotion-standalone-gateway`.
- Production packaging: not yet complete. `agent.workflow.json` and `.github/workflows/package.yml` are required by `GITHUB_WORKFLOW_SPEC.md` before commercial release readiness can be claimed.

## 8. Verification

```bash
pnpm install
pnpm verify
cargo test --workspace
node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
node ../sdkwork-specs/tools/check-agent-workflow-standard.mjs --root .
```

`check-agent-workflow-standard.mjs` is expected to fail until the release workflow governance files are added through human-reviewed release/deployment work.
