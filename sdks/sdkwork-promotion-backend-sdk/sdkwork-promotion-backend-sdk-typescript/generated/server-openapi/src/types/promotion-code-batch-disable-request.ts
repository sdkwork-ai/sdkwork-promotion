export interface PromotionCodeBatchDisableRequest {
  /** Why the batch is being disabled (trimmed, 1..256 characters). The actor is the authenticated operator; no request field can select it. */
  reason: string;
}
