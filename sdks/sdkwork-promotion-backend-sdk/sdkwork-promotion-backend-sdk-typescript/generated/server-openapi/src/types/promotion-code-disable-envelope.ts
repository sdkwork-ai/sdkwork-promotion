import type { PromotionCodeDisable } from './promotion-code-disable';

export interface PromotionCodeDisableEnvelope {
  code: 0;
  data: { item: PromotionCodeDisable; };
  traceId: string;
}
