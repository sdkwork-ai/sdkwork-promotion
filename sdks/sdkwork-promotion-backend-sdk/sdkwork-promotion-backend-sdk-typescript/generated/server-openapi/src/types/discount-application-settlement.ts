export interface DiscountApplicationSettlement {
  /** Settled discount application id. */
  applicationId: string;
  /** Resulting application status; always `settled`. */
  status: string;
  /** Amount that left the reservation pool and became spent, in integer minor units (int64 decimal string). `0` when the application never reserved budget. */
  settledAmountMinor: string;
}
