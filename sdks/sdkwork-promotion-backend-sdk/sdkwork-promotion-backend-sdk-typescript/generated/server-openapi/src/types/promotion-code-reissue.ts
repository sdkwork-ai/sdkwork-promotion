export interface PromotionCodeReissue {
  /** Newly issued replacement code id (`promotion_code.id`, TEXT). */
  codeId: string;
  /** Human-readable number of the replacement (`promotion_code.code_no`). */
  codeNo: string;
  /** Display mask of the replacement; never equal to the redeemable value, which only the audited reveal/export operations return. */
  codeMasked: string;
  /** Last four characters of the replacement. */
  codeLast4: string;
  /** Id of the disabled original (`promotion_code.id`, TEXT). */
  replacedCodeId: string;
}
