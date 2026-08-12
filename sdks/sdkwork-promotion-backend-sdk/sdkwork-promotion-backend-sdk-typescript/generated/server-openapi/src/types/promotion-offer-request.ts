import type { PromotionCouponBenefitRequest } from './promotion-coupon-benefit-request';

export interface PromotionOfferRequest {
  campaignId?: string;
  offerCode?: string | null;
  offerType: string;
  displayName: string;
  description?: string | null;
  audienceScope: string;
  combinability: string;
  goodsScope: string;
  priority: number;
  startsAt: string;
  endsAt?: string | null;
  status: 'active' | 'disabled';
  discountType: string;
  discountValue: string;
  minimumAmount: string;
  maximumDiscountAmount?: string | null;
  currencyCode: string;
  couponBenefit?: PromotionCouponBenefitRequest;
  version?: string;
}
