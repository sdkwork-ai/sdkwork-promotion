import type { PromotionCodeReissue } from './promotion-code-reissue';

export interface PromotionCodeReissueEnvelope {
  code: 0;
  data: { item: PromotionCodeReissue; };
  traceId: string;
}
