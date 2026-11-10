export interface DiscountApplicationRelease {
  /** Released discount application id. */
  applicationId: string;
  /** Resulting application status; always `released`. */
  status: string;
  /** Reservation returned to the available pool, in integer minor units (int64 decimal string). `0` when the application never reserved budget. */
  releasedAmountMinor: string;
}
