import { type SdkworkPromotionAppService } from "@sdkwork/promotion-service";
import { type SdkworkCouponCatalog, type SdkworkUserCoupon, type SdkworkUserCouponDigest, type SdkworkCouponCatalogDigest } from "./coupon";
import { type SdkworkCouponMessagesOverrides } from "./coupon-copy";
export interface SdkworkCouponStatistics {
    expiredCount: number;
    totalCoupons: number;
    unusedCount: number;
    usedCount: number;
}
export interface SdkworkCouponDashboardData {
    availableCoupons: SdkworkUserCoupon[];
    catalogCoupons: SdkworkCouponCatalog[];
    catalogDigest: SdkworkCouponCatalogDigest;
    myCoupons: SdkworkUserCoupon[];
    statistics: SdkworkCouponStatistics;
    userDigest: SdkworkUserCouponDigest;
}
export interface SdkworkCouponRedeemInput {
    channel?: string;
    redeemCode: string;
}
export interface SdkworkCouponPointsExchangeInput {
    couponId: string;
    requestNo?: string;
}
export interface SdkworkCouponRollbackInput {
    reason?: string;
    userCouponId: string;
}
export interface SdkworkCouponUseInput {
    orderId: string;
    userCouponId: string;
}
export interface CreateSdkworkCouponServiceOptions {
    promotionAppService?: SdkworkPromotionAppService;
    locale?: string | null;
    messages?: SdkworkCouponMessagesOverrides;
    pageSize?: number;
}
export interface SdkworkCouponService {
    cancelUseCoupon(userCouponId: string): Promise<SdkworkUserCoupon>;
    exchangeCouponByPoints(input: SdkworkCouponPointsExchangeInput): Promise<SdkworkUserCoupon>;
    getCouponDetail(couponId: string): Promise<SdkworkCouponCatalog>;
    getDashboard(): Promise<SdkworkCouponDashboardData>;
    getEmptyDashboard(): SdkworkCouponDashboardData;
    getUserCouponDetail(userCouponId: string): Promise<SdkworkUserCoupon>;
    receiveCoupon(couponId: string): Promise<SdkworkUserCoupon>;
    redeemCoupon(input: SdkworkCouponRedeemInput): Promise<SdkworkUserCoupon>;
    rollbackPointsExchange(input: SdkworkCouponRollbackInput): Promise<SdkworkUserCoupon>;
    useCoupon(input: SdkworkCouponUseInput): Promise<SdkworkUserCoupon>;
}
export declare function createSdkworkCouponService(options?: CreateSdkworkCouponServiceOptions): SdkworkCouponService;
export declare const sdkworkCouponService: SdkworkCouponService;
//# sourceMappingURL=coupon-service.d.ts.map