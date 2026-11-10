/** One audited reveal result. The full redeemable code is only available through this schema (and the export equivalent), always preceded by an audit-evidence row. */
export interface PromotionCodeReveal {
  /** Human-readable code number (`promotion_code.code_no`). */
  codeNo: string;
  /** Full redeemable promotion code. */
  code: string;
}
