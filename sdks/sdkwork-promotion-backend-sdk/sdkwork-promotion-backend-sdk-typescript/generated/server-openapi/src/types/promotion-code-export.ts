import type { PromotionCodeReveal } from './promotion-code-reveal';

/** One audited batch export. Codes are ordered by `codeNo` and capped at 5000 rows. */
export interface PromotionCodeExport {
  items: PromotionCodeReveal[];
  /** Number of codes actually returned (never more than 5000). */
  count: number;
  /** True when the batch holds more codes than the 5000-row cap. */
  truncated: boolean;
}
