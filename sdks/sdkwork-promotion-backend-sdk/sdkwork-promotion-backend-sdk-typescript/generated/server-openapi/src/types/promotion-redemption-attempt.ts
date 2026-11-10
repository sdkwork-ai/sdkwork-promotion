/** One code-redemption attempt (success or failure). Append-only evidence: rows are never updated or deleted. The submitted code itself is never stored or returned, only its suffix. */
export interface PromotionRedemptionAttempt {
  /** Attempt id (TEXT). */
  id: string;
  /** Resolved promotion code id; null when the submission never matched a code. */
  codeId?: string | null;
  /** Last four characters of the trimmed submitted code. Never the redeemable value. */
  submittedCodeSuffix: string;
  /** Subject that submitted the code, for example `user`. */
  subjectType: string;
  /** Subject id that submitted the code. */
  subjectId: string;
  /** Closed outcome vocabulary. */
  outcome: 'succeeded' | 'failed';
  /** Closed failure taxonomy; always present when `outcome` is `failed` and null when it is `succeeded`. */
  failureCode?: 'code_unavailable' | 'issue_limit_reached' | 'per_user_limit_reached' | 'idempotency_conflict' | 'redemption_in_progress' | 'benefit_invalid' | 'internal_error' | null;
  /** Human-readable rejection message, truncated to 512 characters. Classification lives in the failure code, so clients must not parse this text. */
  failureDetail?: string | null;
  /** Granted benefit kind on success (`points_credit`, `token_bank_credit`, `cash_credit`, `subscription`). */
  benefitKind?: string | null;
  /** Granted amount as a decimal string in the smallest currency unit, or granted points for `points_credit`. */
  grantedAmount?: string | null;
  /** Currency of `grantedAmount`. */
  currencyCode?: string | null;
  /** Caller request number that produced the attempt. */
  requestNo: string;
  /** Order the code was redeemed against, when the attempt came from an order. */
  orderId?: string | null;
  /** Trace id captured from the redemption command. Empty when the caller supplied none. */
  traceId: string;
  /** Instant the attempt was evaluated (server form `YYYY-MM-DD HH:MM:SS`). */
  occurredAt: string;
}
