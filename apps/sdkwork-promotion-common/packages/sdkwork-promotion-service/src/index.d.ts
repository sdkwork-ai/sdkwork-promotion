import { APP_PROMOTION_METHOD_TREE, type ClientFromMethodTree, type PromotionAppSdkClient } from "@sdkwork/promotion-sdk-ports";
import type { SdkworkPromotionMutationStatus } from "@sdkwork/promotion-contracts";
import type { CouponStock, CouponStockRequest, DiscountApplication, PromotionCampaign, PromotionCampaignRequest, PromotionCode, PromotionCodeBatch, PromotionCodeBatchRequest, PromotionCouponLedgerEntry, PromotionDistributionRequest, PromotionDistributionTask, PromotionOffer, PromotionOfferRequest, PromotionOverview, PromotionUserCoupon, SdkworkBackendClient as SdkworkPromotionBackendClient } from "@sdkwork/promotion-backend-sdk";
export type { CouponStock, CouponStockRequest, DiscountApplication, PromotionCampaign, PromotionCampaignRequest, PromotionCode, PromotionCodeBatch, PromotionCodeBatchRequest, PromotionCouponLedgerEntry, PromotionDistributionRequest, PromotionDistributionTask, PromotionOffer, PromotionOfferRequest, PromotionOverview, PromotionUserCoupon, } from "@sdkwork/promotion-backend-sdk";
export interface PromotionAdminListQuery {
    page?: number;
    pageSize?: number;
    q?: string;
    status?: 'active' | 'disabled';
}
export interface PromotionAdminPage<T> {
    items: T[];
    page: number;
    pageSize: number;
    totalItems: number;
    totalPages: number;
}
export interface SdkworkPromotionBackendService {
    getOverview(): Promise<PromotionOverview>;
    listCampaigns(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<PromotionCampaign>>;
    getCampaign(campaignId: string): Promise<PromotionCampaign>;
    createCampaign(input: PromotionCampaignRequest): Promise<PromotionCampaign>;
    updateCampaign(campaignId: string, input: PromotionCampaignRequest): Promise<PromotionCampaign>;
    deleteCampaign(campaignId: string): Promise<void>;
    listOffers(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<PromotionOffer>>;
    getOffer(offerId: string): Promise<PromotionOffer>;
    createOffer(input: PromotionOfferRequest): Promise<PromotionOffer>;
    updateOffer(offerId: string, input: PromotionOfferRequest): Promise<PromotionOffer>;
    updateOfferStatus(offerId: string, status: 'active' | 'disabled'): Promise<void>;
    deleteOffer(offerId: string): Promise<void>;
    listCouponStocks(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<CouponStock>>;
    createCouponStock(input: CouponStockRequest): Promise<CouponStock>;
    listCodeBatches(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<PromotionCodeBatch>>;
    createCodeBatch(input: PromotionCodeBatchRequest): Promise<PromotionCodeBatch>;
    listCodes(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<PromotionCode>>;
    listDistributionTasks(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<PromotionDistributionTask>>;
    createDistributionTask(input: PromotionDistributionRequest): Promise<PromotionDistributionTask>;
    listUserCoupons(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<PromotionUserCoupon>>;
    listCouponLedger(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<PromotionCouponLedgerEntry>>;
    listDiscountApplications(query?: PromotionAdminListQuery): Promise<PromotionAdminPage<DiscountApplication>>;
}
export declare function createSdkworkPromotionBackendService(client: SdkworkPromotionBackendClient): SdkworkPromotionBackendService;
export type SdkworkPromotionPromotionsService = ClientFromMethodTree<(typeof APP_PROMOTION_METHOD_TREE)["promotions"]>;
export type SdkworkPromotionAppService = {
    promotions: SdkworkPromotionPromotionsService;
};
export type SdkworkPromotionAppServiceProvider = () => SdkworkPromotionAppService;
export interface SdkworkPromotionSessionTokens {
    accessToken?: string;
    authToken?: string;
    refreshToken?: string;
}
export type SdkworkPromotionSessionTokenProvider = () => SdkworkPromotionSessionTokens;
export interface CreateSdkworkPromotionAppServiceInput {
    appClient: PromotionAppSdkClient;
}
export interface SdkworkPromotionResponseEnvelope<T> {
    code?: number | string;
    data?: T;
    message?: string;
    msg?: string;
}
export declare function configureSdkworkPromotionAppServiceProvider(provider: SdkworkPromotionAppServiceProvider | null): void;
export declare function configureSdkworkPromotionSessionTokenProvider(provider: SdkworkPromotionSessionTokenProvider | null): void;
export declare function getSdkworkPromotionService(): SdkworkPromotionAppService;
export declare function getSdkworkPromotionSessionTokens(): SdkworkPromotionSessionTokens;
export declare function hasSdkworkPromotionSession(): boolean;
export declare function requireSdkworkPromotionSession(message?: string): void;
export declare function createSdkworkPromotionAppService(input: CreateSdkworkPromotionAppServiceInput): SdkworkPromotionAppService;
export declare function unwrapSdkworkPromotionResponse<T>(value: unknown, fallbackMessage?: string): T;
export declare function toSdkworkPromotionOptionalString(value: unknown): string | undefined;
export declare function toNullableSdkworkPromotionNumber(value: unknown): number | null;
export declare function toSdkworkPromotionNumber(value: unknown, fallback?: number): number;
export declare function toSdkworkPromotionMutationStatus(status: unknown): SdkworkPromotionMutationStatus;
export declare function formatSdkworkPromotionCurrencyCny(value: number | null | undefined, language?: string): string;
export declare function formatSdkworkPromotionPoints(value: number, language?: string): string;
export declare function formatSdkworkPromotionPointsRate(points: number, language?: string): string;
export declare function formatSdkworkPromotionPointsDelta(value: number, language?: string): string;
//# sourceMappingURL=index.d.ts.map