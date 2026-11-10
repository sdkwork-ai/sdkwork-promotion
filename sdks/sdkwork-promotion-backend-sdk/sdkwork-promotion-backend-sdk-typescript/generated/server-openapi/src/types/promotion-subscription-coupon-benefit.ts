export interface PromotionSubscriptionCouponBenefit {
  kind: 'subscription';
  period: 'day' | 'week' | 'month' | 'quarter' | 'year';
  durationDays: string;
  dailyQuota: string;
  totalQuota: string;
  /** Membership product the coupon grants. Required together with skuId and packageId for the order-side redemption path. */
  productId?: string;
  /** Membership SKU the coupon grants. */
  skuId?: string;
  /** Membership package id the coupon grants; must match an active membership package with the declared SKU and duration. */
  packageId?: string;
}
