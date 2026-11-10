import type { PromotionUserCouponVoid } from './promotion-user-coupon-void';

export interface PromotionUserCouponVoidEnvelope {
  code: 0;
  data: { item: PromotionUserCouponVoid; };
  traceId: string;
}
