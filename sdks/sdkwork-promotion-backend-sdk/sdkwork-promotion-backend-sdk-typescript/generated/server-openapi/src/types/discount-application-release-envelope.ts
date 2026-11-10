import type { DiscountApplicationRelease } from './discount-application-release';

export interface DiscountApplicationReleaseEnvelope {
  code: 0;
  data: { item: DiscountApplicationRelease; };
  traceId: string;
}
