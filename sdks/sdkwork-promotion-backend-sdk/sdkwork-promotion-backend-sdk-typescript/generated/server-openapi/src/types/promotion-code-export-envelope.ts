import type { PromotionCodeExport } from './promotion-code-export';

export interface PromotionCodeExportEnvelope {
  code: 0;
  data: { item: PromotionCodeExport; };
  traceId: string;
}
