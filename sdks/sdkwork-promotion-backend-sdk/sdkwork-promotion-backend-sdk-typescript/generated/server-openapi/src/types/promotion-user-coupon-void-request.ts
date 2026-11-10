export interface PromotionUserCouponVoidRequest {
  /** Why the coupon is being voided (trimmed, 1..256 characters). The actor is the authenticated operator; no request field can select it. */
  reason: string;
}
