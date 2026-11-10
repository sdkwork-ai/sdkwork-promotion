export interface CouponStockRequest {
  offerId: string;
  stockType: 'limited' | 'unlimited';
  /** Total quantity. Must be positive for LIMITED stock; 0 allowed for UNLIMITED stock (statistics only) */
  totalQuantity: string;
  perUserLimit: number;
  claimStartsAt?: string | null;
  claimEndsAt?: string | null;
  status: 'active' | 'disabled';
  /** Coupon code issuance mode: REALTIME generates a code at claim time; BATCH dispenses a pre-generated pool code */
  codeIssueMode?: 'realtime' | 'batch';
  /** Optional hard monetary cap for this coupon stock, in integer minor units (int64 decimal string, must be > 0 when present). Omitted means no cap and apply never reserves. Amounts are integer minor units; legacy decimal-major offer amounts are converted at the service boundary and are never stored here. */
  budgetAmountMinor?: string;
}
