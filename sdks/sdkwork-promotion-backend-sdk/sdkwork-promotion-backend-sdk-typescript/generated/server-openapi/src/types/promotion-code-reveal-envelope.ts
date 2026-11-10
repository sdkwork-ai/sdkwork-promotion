import type { PromotionCodeReveal } from './promotion-code-reveal';

export interface PromotionCodeRevealEnvelope {
  code: 0;
  data: { item: PromotionCodeReveal; };
  traceId: string;
}
