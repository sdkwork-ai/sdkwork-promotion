export interface PromotionCodeBatchDisable {
  /** Number of codes this operation disabled (`active` AND never claimed). */
  disabledCount: string;
  /** Codes of the batch the guard skipped (already claimed or no longer active). */
  skippedCount: string;
}
