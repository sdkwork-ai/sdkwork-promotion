# Promotion PRD

Status: active
Owner: SDKWork maintainers
Application: promotion
Updated: 2026-07-08
Specs: REQUIREMENTS_SPEC.md, DOCUMENTATION_SPEC.md

## 1. Background And Problem

Coupons, promotional offers, pricing promotions, and points exchange rules require isolated validation, redemption, and ledger-facing read logic.

This repository is a T1 commerce capability building block. It is self-contained with its own domain logic, persistence, HTTP route builders, standalone gateway, PC application root, and IAM middleware for the promotion capability.

## 2. Target Users

Marketing operators, buyers applying coupons, pricing and offer integrators, and wallet or points integrators.

## 3. Goals And Non-Goals

### Goals

- Own promotion and exchange SQL, coupon routes, offer routes, pricing routes, and points read workflows.
- Keep promotion domain commands separate from order pricing orchestration at the T1 standalone gateway.
- Provide PC promotion surfaces for coupon, offer, pricing, and points workflows through generated or composed SDK/service boundaries.
- Provide backend-admin campaign CRUD, versioned coupon benefits, stock batches, code generation, direct distribution, claims, and ledger audit through the generated backend SDK.
- Fail closed for checkout, payment, points ledger, and account mutation workflows when a real backend or dependency service is unavailable.

### Non-Goals

- Order persistence ownership.
- Payment settlement ownership.
- Account wallet ownership.

## 4. Scope

- Promotion campaigns, versioned coupon benefits, stock and code batches, direct distribution tasks, user coupons, coupon ledger, discount applications, points balance/history, offer display, and pricing promotion surfaces.
- Exchange rule SQL and app HTTP routes such as `/wallet/exchange_rate` and `/wallet/points/exchanges/rules`.
- App promotion router including wallet points read surfaces owned by promotion store.
- Common TypeScript contracts and service ports under `apps/sdkwork-promotion-common/packages/`.
- PC application packages under `apps/sdkwork-promotion-pc/packages/`.

Primary API prefixes:

- App: `/app/v3/api/promotions`
- Backend: `/backend/v3/api/promotions`

Migration status: complete for active promotion ownership. Release workflow governance remains a pre-launch gap until `agent.workflow.json` and `.github/workflows/package.yml` are approved and added.

## 5. User Scenarios

- A buyer lists available coupons and applies one during checkout orchestrated by the order capability.
- A wallet integrator reads points-to-cash exchange rates from the promotion app router.
- A PC user opens coupon, offer, pricing, and points pages without feature packages bypassing SDK/service boundaries.
- A marketing operator creates a campaign and coupon benefit, allocates stock, generates a bounded code batch, or issues coupons directly to selected users.
- A marketing auditor reviews masked codes, user coupon claims, distribution outcomes, stock ledger entries, and discount applications within the current tenant and organization.

## 6. Success Metrics

- Promotion SQL and HTTP routes are owned exclusively in `sdkwork-promotion`.
- Coupon and exchange routes are served from `sdkwork-routes-promotion-app-api`.
- PC feature packages consume SDKWork APIs through composed SDK/service boundaries and pass package typecheck.
- Repository verification passes for API envelopes, pagination, SDK imports, package scripts, and application composition.

## 7. Phases

- Phase 1 (complete): domain service, SQL, exchange SQL, and app promotion router owned in `sdkwork-promotion`.
- Phase 2 (complete): exchange HTTP routes owned by promotion app router.
- Phase 3 (in progress): PC package dependency closure, service contract alignment, and documentation cleanup.
- Phase 3a (complete): backend-admin campaign, coupon inventory, code batch, distribution, claim, and ledger management closure.
- Phase 4 (requires human release review): GitHub packaging workflow and release evidence.

## 8. Linked Requirements

- Repository standards: `../sdkwork-specs/README.md`
- Component contract: `specs/component.spec.json`
- Machine contracts: local `specs/`, `apis/`, and `sdks/`
- Canon architecture: `docs/architecture/tech/TECH_ARCHITECTURE.md`

## 9. Open Questions

- Release packaging target matrix and publication policy require human review before adding workflow governance files.
