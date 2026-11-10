export interface PromotionUserCouponExtensionRequest {
  /** New expiry, RFC 3339 (`2026-12-01T00:00:00.000Z`) or the canonical server form (`2026-12-01 00:00:00`). Must be later than the database clock. */
  expiresAt: string;
}
