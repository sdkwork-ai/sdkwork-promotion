export interface SdkworkAppCapabilityManifest {
    description: string;
    host?: string;
    id: string;
    packageNames: string[];
    theme?: string;
    title: string;
}
export interface CreateSdkworkAppCapabilityManifestOptions {
    description?: string;
    host?: string;
    id?: string;
    packageNames?: string[];
    theme?: string;
    title?: string;
}
export type SdkworkCouponTab = "discover" | "history" | "my";
export type SdkworkCouponStatus = "available" | "expired" | "inactive" | "used";
export type SdkworkCouponType = "cash" | "discount" | "gift" | "points-exchange" | "unknown";
export type SdkworkCouponAcquireType = "admin-grant" | "points-exchange" | "receive" | "redeem-code" | "unknown";
export interface SdkworkCouponWorkspaceManifest extends SdkworkAppCapabilityManifest {
    capability: "coupon";
    routePath: string;
}
export interface CreateCouponWorkspaceManifestOptions extends Partial<Pick<CreateSdkworkAppCapabilityManifestOptions, "description" | "host" | "id" | "packageNames" | "theme" | "title">> {
    routePath?: string;
}
export interface SdkworkCouponRouteIntent {
    couponId?: string;
    focusWindow: boolean;
    route: string;
    source: "coupon-workspace";
    tab?: SdkworkCouponTab;
    type: "coupon-route-intent";
    userCouponId?: string;
}
export interface CreateCouponRouteIntentOptions {
    basePath?: string;
    couponId?: string;
    focusWindow?: boolean;
    tab?: SdkworkCouponTab;
    userCouponId?: string;
}
export interface SdkworkCouponCatalog {
    amountCny: number | null;
    canReceive: boolean;
    couponId?: string;
    description?: string;
    discountRate: number | null;
    endTime?: string;
    getLimit: number | null;
    id: string;
    minimumSpendCny: number | null;
    name: string;
    pointCost: number | null;
    pointsExchange: boolean;
    receivedCount: number | null;
    remainingCount: number | null;
    scopeType?: string;
    scopeValue?: string;
    stackable: boolean;
    startTime?: string;
    status: SdkworkCouponStatus;
    statusLabel?: string;
    total: number | null;
    type: SdkworkCouponType;
    typeLabel?: string;
    usedCount: number | null;
}
export interface SdkworkUserCoupon {
    acquireAt?: string;
    acquireType?: SdkworkCouponAcquireType;
    amountCny: number | null;
    available: boolean;
    code?: string;
    couponId?: string;
    expireAt?: string;
    id: string;
    minimumSpendCny: number | null;
    name: string;
    orderId?: string;
    pointCost: number | null;
    pointsRefundAt?: string;
    pointsRefunded: boolean;
    remainingDays: number | null;
    scopeType?: string;
    scopeValue?: string;
    status: SdkworkCouponStatus;
    statusLabel?: string;
    type: SdkworkCouponType;
    typeLabel?: string;
    useAt?: string;
    userCouponId?: string;
    discountRate?: number | null;
}
export interface SdkworkCouponCatalogDigestInput {
    canReceive: boolean;
    id: string;
    pointsExchange: boolean;
    status: SdkworkCouponStatus;
}
export interface SdkworkCouponCatalogDigest {
    claimableCoupons: number;
    pointsExchangeCoupons: number;
    totalCoupons: number;
}
export interface SdkworkUserCouponDigestInput {
    discountAmountCny?: number | null;
    id: string;
    remainingDays?: number | null;
    status: SdkworkCouponStatus;
}
export interface SdkworkUserCouponDigest {
    availableCoupons: number;
    expiringSoonCoupons: number;
    highestDiscountAmountCny: number;
    totalCoupons: number;
}
export interface SdkworkCouponDiscountInput {
    discountAmountCny?: number | null;
    discountRate?: number | null;
    id: string;
    minimumSpendCny?: number | null;
    name: string;
    status?: SdkworkCouponStatus;
}
export interface SdkworkRemoteCouponLike {
    amount?: unknown;
    canReceive?: unknown;
    couponId?: unknown;
    description?: unknown;
    discount?: unknown;
    getLimit?: unknown;
    minConsume?: unknown;
    name?: unknown;
    pointCost?: unknown;
    pointsExchange?: unknown;
    receivedCount?: unknown;
    remainingCount?: unknown;
    scopeType?: unknown;
    scopeValue?: unknown;
    stackable?: unknown;
    startTime?: unknown;
    endTime?: unknown;
    status?: unknown;
    statusName?: unknown;
    total?: unknown;
    type?: unknown;
    typeName?: unknown;
    usedCount?: unknown;
}
export interface SdkworkRemoteUserCouponLike {
    acquireAt?: unknown;
    acquireType?: unknown;
    amount?: unknown;
    available?: unknown;
    couponCode?: unknown;
    couponId?: unknown;
    couponName?: unknown;
    discount?: unknown;
    expireAt?: unknown;
    minConsume?: unknown;
    orderId?: unknown;
    pointCost?: unknown;
    pointsRefundAt?: unknown;
    pointsRefunded?: unknown;
    remainingDays?: unknown;
    scopeType?: unknown;
    scopeValue?: unknown;
    status?: unknown;
    statusName?: unknown;
    type?: unknown;
    typeName?: unknown;
    useAt?: unknown;
    userCouponId?: unknown;
}
export declare function normalizeSdkworkCouponType(value: unknown): SdkworkCouponType;
export declare function normalizeSdkworkCouponAcquireType(value: unknown): SdkworkCouponAcquireType;
export declare function normalizeSdkworkCouponStatus(value: unknown, available?: unknown): SdkworkCouponStatus;
export declare function normalizeSdkworkRemoteCoupon(coupon: SdkworkRemoteCouponLike, index?: number): SdkworkCouponCatalog;
export declare function normalizeSdkworkRemoteUserCoupon(coupon: SdkworkRemoteUserCouponLike, index?: number): SdkworkUserCoupon;
export declare function sortSdkworkCouponCatalog(coupons: readonly SdkworkCouponCatalog[]): SdkworkCouponCatalog[];
export declare function sortSdkworkUserCoupons(coupons: readonly SdkworkUserCoupon[]): SdkworkUserCoupon[];
export declare function estimateSdkworkCouponDiscountAmount(amountCny: number, coupon: SdkworkCouponDiscountInput | null | undefined): number;
export declare function summarizeSdkworkCouponCatalog(coupons: readonly SdkworkCouponCatalogDigestInput[]): SdkworkCouponCatalogDigest;
export declare function summarizeSdkworkUserCoupons(coupons: readonly SdkworkUserCouponDigestInput[]): SdkworkUserCouponDigest;
export declare function resolveSdkworkUserCouponRequestId(coupon: Pick<SdkworkUserCoupon, "couponId" | "id" | "userCouponId">): string | undefined;
export declare function createCouponWorkspaceManifest({ description, host, id, packageNames, routePath, theme, title, }?: CreateCouponWorkspaceManifestOptions): SdkworkCouponWorkspaceManifest;
export declare function createCouponRouteIntent(options?: CreateCouponRouteIntentOptions): SdkworkCouponRouteIntent;
export declare const couponPackageMeta: {
    readonly architecture: "pc-react";
    readonly domain: "commerce";
    readonly package: "@sdkwork/promotion-pc-coupon";
    readonly status: "ready";
};
export type CouponPackageMeta = typeof couponPackageMeta;
//# sourceMappingURL=coupon.d.ts.map