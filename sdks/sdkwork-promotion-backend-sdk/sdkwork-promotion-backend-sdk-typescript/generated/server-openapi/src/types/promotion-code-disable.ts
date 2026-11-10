export interface PromotionCodeDisable {
  /** Disabled promotion code id (`promotion_code.id`, TEXT). */
  codeId: string;
  /** Resulting code status; always `disabled`. */
  status: string;
}
