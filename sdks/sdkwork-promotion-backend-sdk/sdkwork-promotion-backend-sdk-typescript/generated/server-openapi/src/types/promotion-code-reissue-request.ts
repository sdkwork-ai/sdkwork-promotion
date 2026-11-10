export interface PromotionCodeReissueRequest {
  /** Why the code is being reissued (trimmed, 1..256 characters). The actor is the authenticated operator; no request field can select it. */
  reason: string;
  /** Optional explicit replacement code. Trimmed and upper-cased before storage; 4–64 characters of `A-Z0-9-_`. Must be unique in the tenant (`409` on collision). Omit to generate the value from the batch's `codePrefix`/`codeLength`. */
  code?: string;
}
