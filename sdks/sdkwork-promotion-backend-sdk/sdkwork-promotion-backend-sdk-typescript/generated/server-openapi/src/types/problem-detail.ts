export interface ProblemDetail {
  type: string;
  title: string;
  status: number;
  detail: string;
  code: number;
  traceId: string;
  /** Stable SDKWork message key a client can localize instead of rendering `detail` verbatim. Display metadata only; never used for control flow. */
  i18nKey?: string;
  /** BCP 47 locale the server rendered `detail` in. Display metadata only. */
  locale?: string;
  /** Request path the problem was raised for (RFC 9457 `instance`). */
  instance?: string;
  /** Operation that raised the problem, for example `redemptionAttempts.list`. */
  operationId?: string;
}
