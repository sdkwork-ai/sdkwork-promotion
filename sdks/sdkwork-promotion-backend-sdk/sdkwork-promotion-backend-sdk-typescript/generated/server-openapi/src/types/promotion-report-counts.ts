export interface PromotionReportCounts {
  /** Promotion codes created in the window (windowed by `promotion_code.created_at`). Shared denominator of every rate. */
  issuedCodes: string;
  /** Wallet coupons claimed in the window (windowed by `promotion_user_coupon.claimed_at`). */
  claimedCoupons: string;
  /** Wallet coupons claimed in the window that reached `redeemed`/`used`. */
  redeemedCoupons: string;
  /** Wallet coupons claimed in the window that expired. */
  expiredCoupons: string;
  /** Wallet coupons claimed in the window that an operator voided. */
  voidedCoupons: string;
  /** Promotion codes created in the window that are currently `disabled`. */
  disabledCodes: string;
  /** Discount applications created in the window (windowed by `promotion_discount_application.applied_at`), any status. */
  applicationsApplied: string;
  /** Discount applications created in the window that settled. */
  applicationsSettled: string;
  /** Discount applications created in the window that were released. */
  applicationsReleased: string;
}
