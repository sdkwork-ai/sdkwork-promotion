import type { PromotionUserCouponExtension } from './promotion-user-coupon-extension';

export interface PromotionUserCouponExtensionEnvelope {
  code: 0;
  data: { item: PromotionUserCouponExtension; };
  traceId: string;
}
