export interface PromotionCodeBatchRequest {
  stockId: string;
  codeType: string;
  codeLength?: number;
  codePrefix?: string;
  /** Full coupon codes defined by the admin. When present, the batch stores these codes verbatim (trimmed and upper-cased) instead of generating prefix plus random suffixes; the array length must equal requestedQuantity and codes must be unique. */
  customCodes?: string[];
  /** Redemption cap per generated code. 1 means single use; larger values issue a shared multi-use code. */
  maxClaims?: number;
  startsAt?: string | null;
  expiresAt?: string | null;
  idempotencyKey: string;
  requestedQuantity: string;
}
