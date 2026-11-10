import type { PromotionCodeBatchDisable } from './promotion-code-batch-disable';

export interface PromotionCodeBatchDisableEnvelope {
  code: 0;
  data: { item: PromotionCodeBatchDisable; };
  traceId: string;
}
