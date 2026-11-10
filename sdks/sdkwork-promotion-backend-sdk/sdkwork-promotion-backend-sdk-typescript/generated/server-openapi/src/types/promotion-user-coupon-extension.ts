export interface PromotionUserCouponExtension {
  /** Extended wallet coupon id. */
  userCouponId: string;
  /** New expiry in the canonical RFC 3339 UTC shape (`YYYY-MM-DDTHH:MM:SS.sssZ`). */
  expiresAt: string;
}
