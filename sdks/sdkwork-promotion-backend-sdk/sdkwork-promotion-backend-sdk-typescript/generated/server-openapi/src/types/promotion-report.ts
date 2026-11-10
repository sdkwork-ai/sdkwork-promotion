import type { PromotionReportCounts } from './promotion-report-counts';
import type { PromotionReportCurrencyTotal } from './promotion-report-currency-total';
import type { PromotionReportRates } from './promotion-report-rates';
import type { PromotionReportTopOffer } from './promotion-report-top-offer';

export interface PromotionReport {
  /** Inclusive start of the report window, canonical RFC 3339 UTC. */
  from: string;
  /** Inclusive end of the report window, canonical RFC 3339 UTC. */
  to: string;
  counts: PromotionReportCounts;
  rates: PromotionReportRates;
  /** Money totals, one row per currency. Money is never summed across currencies. */
  byCurrency: PromotionReportCurrencyTotal[];
  /** Offers with the most wallet coupons redeemed in the window, tie-broken by settled discount amount then offer id. */
  topOffers: PromotionReportTopOffer[];
}
