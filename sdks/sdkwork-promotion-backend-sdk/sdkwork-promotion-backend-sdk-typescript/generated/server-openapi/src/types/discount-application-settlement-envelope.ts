import type { DiscountApplicationSettlement } from './discount-application-settlement';

export interface DiscountApplicationSettlementEnvelope {
  code: 0;
  data: { item: DiscountApplicationSettlement; };
  traceId: string;
}
