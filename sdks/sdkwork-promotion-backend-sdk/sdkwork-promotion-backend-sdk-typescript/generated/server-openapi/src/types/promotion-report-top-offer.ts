export interface PromotionReportTopOffer {
  offerId: string;
  offerNo: string;
  displayName: string;
  /** Wallet coupons of this offer redeemed in the window. */
  redeemed: string;
  /** Settled discount amount of this offer in `currencyCode` inside the window, in integer minor units. */
  discountSettledMinor: string;
  /** The single currency this row's amounts belong to (the currency of the offer's settled amount). */
  currencyCode: string;
}
