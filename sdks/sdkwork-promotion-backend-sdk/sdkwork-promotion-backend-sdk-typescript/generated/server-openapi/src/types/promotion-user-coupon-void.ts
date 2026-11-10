export interface PromotionUserCouponVoid {
  /** Voided wallet coupon id (`promotion_user_coupon.id`, TEXT). */
  userCouponId: string;
  /** Resulting coupon status; always `voided`. */
  status: string;
}
