export type SdkworkCouponLocale = "en-US" | "zh-CN";
export type SdkworkCouponMessagesOverrides = DeepPartial<SdkworkCouponMessages>;
export interface SdkworkCouponMessages {
    actions: {
        cancelUse: string;
        claimCoupon: string;
        close: string;
        discover: string;
        exchangePoints: string;
        history: string;
        myCoupons: string;
        redeemCode: string;
        refreshInventory: string;
        rollbackPoints: string;
        viewDetails: string;
    };
    common: {
        available: string;
        coupon: string;
        days: string;
        emptyValue: string;
        no: string;
        pointCost: string;
        yes: string;
    };
    controller: {
        bootstrapFailed: string;
        cancelUseFailed: string;
        couponDetailFailed: string;
        exchangeFailed: string;
        receiveFailed: string;
        redeemFailed: string;
        rollbackFailed: string;
        selectCouponRequired: string;
        useFailed: string;
        userCouponDetailFailed: string;
    };
    detail: {
        acquiredAtLabel: string;
        availabilityLabel: string;
        catalogEyebrow: string;
        couponIdLabel: string;
        discountMetricLabel: string;
        expireAtLabel: string;
        loading: string;
        minimumSpendLabel: string;
        ownedEyebrow: string;
        orderIdLabel: string;
        overviewDescription: string;
        overviewTitle: string;
        pointCostLabel: string;
        remainingDaysLabel: string;
        stackableLabel: string;
        statusMetricLabel: string;
        summaryFallback: string;
        title: string;
        typeLabel: string;
        usageDescription: string;
        usageTitle: string;
        useAtLabel: string;
        userCouponIdLabel: string;
    };
    page: {
        activeCouponFallback: string;
        claimableOffersLabel: string;
        description: string;
        errorTitle: string;
        eyebrow: string;
        highestDiscountLabel: string;
        inventoryEyebrow: string;
        inventoryTitle: string;
        loading: string;
        title: string;
    };
    format: {
        pointCostValue: string;
        remainingDaysValue: string;
    };
    inventory: {
        catalogFallbackDescription: string;
        codeLabel: string;
        emptyDiscover: string;
        emptyVisible: string;
        pointCostLabel: string;
        remainingDaysLabel: string;
    };
    redeemDialog: {
        benefitCheckout: string;
        benefitInventory: string;
        benefitRecovery: string;
        description: string;
        eyebrow: string;
        errorTitle: string;
        inputLabel: string;
        inputPlaceholder: string;
        previewLabel: string;
        summaryDescription: string;
        summaryTitle: string;
        title: string;
    };
    service: {
        cancelUseFailed: string;
        couponDetailFailed: string;
        exchangeFailed: string;
        receiveFailed: string;
        redeemFailed: string;
        requestFailed: string;
        rollbackFailed: string;
        signInRequired: string;
        useFailed: string;
        userCouponDetailFailed: string;
    };
    stats: {
        availableCoupons: string;
        claimableOffers: string;
        expiringSoon: string;
        highestDiscount: string;
        pointsExchangeOffers: string;
        totalInventory: string;
        usedCoupons: string;
    };
    status: {
        available: string;
        expired: string;
        inactive: string;
        used: string;
    };
    type: {
        cash: string;
        discount: string;
        gift: string;
        pointsExchange: string;
        unknown: string;
    };
}
type DeepPartial<T> = {
    [K in keyof T]?: T[K] extends (...args: never[]) => unknown ? T[K] : T[K] extends object ? DeepPartial<T[K]> : T[K];
};
export declare function normalizeSdkworkCouponLocale(locale?: string | null): SdkworkCouponLocale;
export declare function createSdkworkCouponMessages(locale?: string | null, overrides?: SdkworkCouponMessagesOverrides): SdkworkCouponMessages;
export {};
//# sourceMappingURL=coupon-copy.d.ts.map