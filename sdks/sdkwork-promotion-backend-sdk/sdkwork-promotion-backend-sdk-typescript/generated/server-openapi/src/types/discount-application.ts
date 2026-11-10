export interface DiscountApplication {
  id: string;
  applicationNo: string;
  orderId: string;
  orderNo?: string | null;
  offerId: string;
  /** Monetary discount kind: NONE (benefit-only coupon, no order discount), FIXED (fixed amount off), or PERCENTAGE (percent off). */
  discountType: string;
  /** Computed discount actually taken off the order, in integer minor units (currency minor unit, e.g. cents for CNY). */
  discountAmount: string;
  currencyCode: string;
  status: 'applied' | 'settled' | 'released' | 'rolled_back';
  appliedAt: string;
  settledAt?: string | null;
  releasedAt?: string | null;
  rolledBackAt?: string | null;
}
