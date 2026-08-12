import type { NoData } from './no-data';

/** Result schema exposed by Cloud Router. */
export interface PointsExchangeRulesResult {
  code: 0;
  data: unknown & NoData;
  /** Server-owned request correlation id. */
  traceId: string;
}
