/** Integer ratios in basis points (100% = 10000), all over the issued-code denominator; 0 when the denominator is 0, clamped to 0..10000. */
export interface PromotionReportRates {
  /** `claimedCoupons * 10000 / issuedCodes`, integer division (round down). */
  claimRateBp: string;
  /** `redeemedCoupons * 10000 / issuedCodes`, integer division (round down). */
  redemptionRateBp: string;
  /** `voidedCoupons * 10000 / issuedCodes`, integer division (round down). */
  voidRateBp: string;
}
