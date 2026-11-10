export interface PromotionReportCurrencyTotal {
  /** ISO 4217 currency code. Each row totals only its own currency. */
  currencyCode: string;
  /** Sum of `promotion_discount_application.discount_amount` for this currency inside the window, in integer minor units. */
  discountAppliedMinor: string;
  /** Sum of discount amounts of applications in this currency and status `settled` inside the window, in integer minor units. */
  discountSettledMinor: string;
  /** Current snapshot (not window-filtered): sum of `promotion_budget_account.total_amount_minor` for this currency, in integer minor units. */
  budgetTotalMinor: string;
  /** Current snapshot (not window-filtered): sum of `promotion_budget_account.reserved_amount_minor` for this currency, in integer minor units. */
  budgetReservedMinor: string;
  /** Current snapshot (not window-filtered): sum of `promotion_budget_account.used_amount_minor` for this currency, in integer minor units. */
  budgetUsedMinor: string;
}
