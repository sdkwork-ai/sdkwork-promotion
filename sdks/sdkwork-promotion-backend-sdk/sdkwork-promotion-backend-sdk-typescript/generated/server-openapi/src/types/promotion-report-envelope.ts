import type { PromotionReport } from './promotion-report';

export interface PromotionReportEnvelope {
  code: 0;
  data: unknown & { item: PromotionReport; };
  traceId: string;
}
