import type { PageInfo } from './page-info';
import type { PromotionRedemptionAttempt } from './promotion-redemption-attempt';

export interface PromotionRedemptionAttemptListEnvelope {
  code: 0;
  data: { items: PromotionRedemptionAttempt[]; pageInfo: PageInfo; };
  traceId: string;
}
