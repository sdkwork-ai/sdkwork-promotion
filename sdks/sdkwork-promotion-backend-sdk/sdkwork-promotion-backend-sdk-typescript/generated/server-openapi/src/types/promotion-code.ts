export interface PromotionCode {
  id: string;
  stockId: string;
  offerId: string;
  codeNo: string;
  /** Full redeemable code. It is still stored plaintext in this phase because the audited `codes.reveals.create` / `codeBatches.codeExports.create` operations return it, so it is not removed yet; uniqueness and redemption lookup use the non-reversible hash of `UPPER(TRIM(code))` (`promotion_code.code_hash`) instead of this value. Deprecated as a list-response field: prefer `codeMasked`/`codeLast4` for display and the audited reveal/export operations for the full value. */
  promotionCode: string;
  /** Masked display form of `promotionCode`: the first four and last four characters when the code has at least 9 characters (`3C76****4E21`), otherwise every character except the last two is masked. Never equal to the full code. */
  codeMasked: string;
  /** Last four characters of the code (fewer when the code is shorter). A stable, non-redeemable fragment safe to show or quote to support. */
  codeLast4: string;
  codeType: string;
  maxClaims: number;
  claimedQuantity: number;
  startsAt?: string | null;
  expiresAt?: string | null;
  status: 'active' | 'disabled';
}
