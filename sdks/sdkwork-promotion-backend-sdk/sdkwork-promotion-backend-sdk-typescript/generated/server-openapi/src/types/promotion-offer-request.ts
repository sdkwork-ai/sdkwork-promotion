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
  /** Discount calculation mode. Allowed values: `FIXED` (fixed amount off), `PERCENTAGE` (percentage off; `discountValue` is the percentage), `NONE` (benefit-only coupon; no order discount). Input is case-insensitive; the stored value is upper case. */
  discountType: string;
  /** Discount value. `FIXED`: the amount off in decimal major units. `PERCENTAGE`: the percentage in the range (0, 100], where `12.5` means 12.50%. `NONE`: must be `0`. Plain non-negative decimal with at most 2 fraction digits: no sign, exponent, comma, or extra precision. */
  discountValue: string;
  /** Minimum order amount this coupon requires, in decimal major units (`0` means no minimum). Plain non-negative decimal with at most 2 fraction digits. */
  minimumAmount: string;
  /** Optional cap on a `PERCENTAGE` discount, in decimal major units. Must be greater than `0`, and is rejected for `FIXED` and `NONE`. */
  maximumDiscountAmount?: string | null;
  currencyCode: string;
  couponBenefit?: PromotionCouponBenefitRequest;
  version?: string;
}
