export interface DiscountApplicationReleaseRequest {
  /** Why the reservation is being released (trimmed, 1..256 characters). The actor is the authenticated operator; no request field can select it. */
  reason: string;
}
