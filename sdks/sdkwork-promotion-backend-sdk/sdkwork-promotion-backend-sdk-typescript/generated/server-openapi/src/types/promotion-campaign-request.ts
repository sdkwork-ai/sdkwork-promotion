export interface PromotionCampaignRequest {
  campaignCode?: string | null;
  displayName: string;
  description?: string | null;
  channelScope: string;
  audienceScope: string;
  startsAt: string;
  endsAt?: string | null;
  status: string;
  version?: string;
}
