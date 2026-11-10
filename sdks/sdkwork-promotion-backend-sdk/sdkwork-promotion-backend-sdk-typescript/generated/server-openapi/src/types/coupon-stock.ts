export interface CouponStock {
  id: string;
  offerId: string;
  stockNo: string;
  stockType: 'limited' | 'unlimited';
  totalQuantity: string;
  availableQuantity: string;
  claimedQuantity: string;
  redeemedQuantity: string;
  lockedQuantity: string;
  perUserLimit: number;
  claimStartsAt?: string | null;
  claimEndsAt?: string | null;
  status: 'active' | 'disabled';
  codeIssueMode: 'realtime' | 'batch';
  /** Hard monetary cap of the stock's budget account in integer minor units (int64 decimal string). Null when the stock has no budget account. */
  budgetAmountMinor?: string | null;
  /** Amount of the stock budget still available to reserve, in integer minor units (int64 decimal string). Null when the stock has no budget account. */
  budgetAvailableMinor?: string | null;
  /** Amount of the stock budget currently held by live reservations, in integer minor units (int64 decimal string). Null when the stock has no budget account. */
  budgetReservedMinor?: string | null;
  /** Amount of the stock budget already spent, in integer minor units (int64 decimal string). Null when the stock has no budget account. */
  budgetUsedMinor?: string | null;
}
