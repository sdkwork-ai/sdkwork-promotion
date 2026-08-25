import { getSdkworkPromotionService, hasSdkworkPromotionSession, requireSdkworkPromotionSession, toNullableSdkworkPromotionNumber, toSdkworkPromotionOptionalString, unwrapSdkworkPromotionResponse, } from "@sdkwork/promotion-service";
import { normalizeSdkworkRemoteCoupon, normalizeSdkworkRemoteUserCoupon, sortSdkworkCouponCatalog, sortSdkworkUserCoupons, summarizeSdkworkCouponCatalog, summarizeSdkworkUserCoupons, } from "./coupon";
import { createSdkworkCouponMessages, } from "./coupon-copy";
function createEmptyDashboard() {
    return {
        availableCoupons: [],
        catalogCoupons: [],
        catalogDigest: summarizeSdkworkCouponCatalog([]),
        myCoupons: [],
        statistics: {
            expiredCount: 0,
            totalCoupons: 0,
            unusedCount: 0,
            usedCount: 0,
        },
        userDigest: summarizeSdkworkUserCoupons([]),
    };
}
function mapStatistics(statistics) {
    return {
        expiredCount: toNullableSdkworkPromotionNumber(statistics?.expiredCount) ?? 0,
        totalCoupons: toNullableSdkworkPromotionNumber(statistics?.totalCoupons) ?? 0,
        unusedCount: toNullableSdkworkPromotionNumber(statistics?.unusedCount) ?? 0,
        usedCount: toNullableSdkworkPromotionNumber(statistics?.usedCount) ?? 0,
    };
}
function mergeUserCoupon(coupon, fallback) {
    if (!fallback) {
        return coupon;
    }
    return {
        ...fallback,
        ...coupon,
        amountCny: coupon.amountCny ?? fallback.amountCny,
        couponId: coupon.couponId ?? fallback.couponId,
        minimumSpendCny: coupon.minimumSpendCny ?? fallback.minimumSpendCny,
        pointCost: coupon.pointCost ?? fallback.pointCost,
        userCouponId: coupon.userCouponId ?? fallback.userCouponId,
    };
}
function stripPrefixedId(value, prefix) {
    return value.startsWith(prefix) ? value.slice(prefix.length) : value;
}
function createRequestNo(prefix) {
    return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}
export function createSdkworkCouponService(options = {}) {
    const copy = createSdkworkCouponMessages(options.locale, options.messages).service;
    const getCommerceService = () => options.promotionAppService ?? getSdkworkPromotionService();
    const pageSize = options.pageSize ?? 20;
    return {
        async cancelUseCoupon(userCouponId) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const normalizedUserCouponId = stripPrefixedId(userCouponId, "user-coupon-");
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.discountApplications.reversals.create({
                userCouponId: normalizedUserCouponId,
            }), copy.cancelUseFailed);
            return normalizeSdkworkRemoteUserCoupon(result);
        },
        async exchangeCouponByPoints(input) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const couponId = stripPrefixedId(input.couponId, "coupon-");
            const requestNo = input.requestNo ?? createRequestNo("coupon-points");
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.userCoupons.claims.create({
                offerId: couponId,
                requestNo,
                sourceType: "points_exchange",
            }), copy.exchangeFailed);
            return normalizeSdkworkRemoteUserCoupon(result);
        },
        async getCouponDetail(couponId) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const normalizedCouponId = stripPrefixedId(couponId, "coupon-");
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.offers.retrieve(normalizedCouponId), copy.couponDetailFailed);
            return normalizeSdkworkRemoteCoupon(result);
        },
        async getDashboard() {
            if (!hasSdkworkPromotionSession()) {
                return createEmptyDashboard();
            }
            const [catalogPayload, myPayload, availablePayload] = await Promise.all([
                getCommerceService().promotions.offers.list({
                    page: 1,
                    page_size: pageSize,
                }),
                getCommerceService().promotions.userCoupons.wallet.list({
                    page: 1,
                    page_size: pageSize,
                }),
                getCommerceService().promotions.userCoupons.wallet.list({
                    page: 1,
                    page_size: pageSize,
                    status: "available",
                }),
            ]);
            const catalogPage = unwrapSdkworkPromotionResponse(catalogPayload, copy.requestFailed);
            const myPage = unwrapSdkworkPromotionResponse(myPayload, copy.requestFailed);
            const availablePage = unwrapSdkworkPromotionResponse(availablePayload, copy.requestFailed);
            const catalogCoupons = sortSdkworkCouponCatalog((catalogPage.content ?? []).map((coupon, index) => normalizeSdkworkRemoteCoupon(coupon, index)));
            const myCoupons = sortSdkworkUserCoupons((myPage.content ?? []).map((coupon, index) => normalizeSdkworkRemoteUserCoupon(coupon, index)));
            const availableCouponsFromRemote = sortSdkworkUserCoupons((availablePage.content ?? []).map((coupon, index) => normalizeSdkworkRemoteUserCoupon(coupon, index)));
            const myCouponsByKey = new Map(myCoupons.flatMap((coupon) => [
                [coupon.userCouponId, coupon],
                [coupon.couponId, coupon],
                [coupon.id, coupon],
            ]).filter((entry) => Boolean(entry[0])));
            const availableCoupons = availableCouponsFromRemote.length > 0
                ? availableCouponsFromRemote.map((coupon) => mergeUserCoupon(coupon, myCouponsByKey.get(coupon.userCouponId ?? "")
                    ?? myCouponsByKey.get(coupon.couponId ?? "")
                    ?? myCouponsByKey.get(coupon.id)))
                : myCoupons.filter((coupon) => coupon.status === "available");
            const statistics = {
                expiredCount: myCoupons.filter((coupon) => coupon.status === "expired").length,
                totalCoupons: myCoupons.length,
                unusedCount: myCoupons.filter((coupon) => coupon.status === "available").length,
                usedCount: myCoupons.filter((coupon) => coupon.status === "used").length,
            };
            return {
                availableCoupons,
                catalogCoupons,
                catalogDigest: summarizeSdkworkCouponCatalog(catalogCoupons),
                myCoupons,
                statistics: mapStatistics(statistics),
                userDigest: summarizeSdkworkUserCoupons(myCoupons.map((coupon) => ({
                    discountAmountCny: coupon.amountCny,
                    id: coupon.id,
                    remainingDays: coupon.remainingDays,
                    status: coupon.status,
                }))),
            };
        },
        getEmptyDashboard() {
            return createEmptyDashboard();
        },
        async getUserCouponDetail(userCouponId) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const normalizedUserCouponId = stripPrefixedId(userCouponId, "user-coupon-");
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.userCoupons.wallet.retrieve(normalizedUserCouponId), copy.userCouponDetailFailed);
            return normalizeSdkworkRemoteUserCoupon(result);
        },
        async receiveCoupon(couponId) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const normalizedCouponId = stripPrefixedId(couponId, "coupon-");
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.userCoupons.claims.create({ offerId: normalizedCouponId }), copy.receiveFailed);
            return normalizeSdkworkRemoteUserCoupon(result);
        },
        async redeemCoupon(input) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.codes.redemptions.create({
                channel: toSdkworkPromotionOptionalString(input.channel),
                code: input.redeemCode,
            }), copy.redeemFailed);
            return normalizeSdkworkRemoteUserCoupon(result);
        },
        async rollbackPointsExchange(input) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const normalizedUserCouponId = stripPrefixedId(input.userCouponId, "user-coupon-");
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.discountApplications.reversals.create({
                reason: toSdkworkPromotionOptionalString(input.reason),
                userCouponId: normalizedUserCouponId,
            }), copy.rollbackFailed);
            return normalizeSdkworkRemoteUserCoupon(result);
        },
        async useCoupon(input) {
            requireSdkworkPromotionSession(copy.signInRequired);
            const normalizedUserCouponId = stripPrefixedId(input.userCouponId, "user-coupon-");
            const result = unwrapSdkworkPromotionResponse(await getCommerceService().promotions.discountApplications.create({
                orderId: input.orderId,
                userCouponId: normalizedUserCouponId,
            }), copy.useFailed);
            return normalizeSdkworkRemoteUserCoupon(result);
        },
    };
}
export const sdkworkCouponService = createSdkworkCouponService();
//# sourceMappingURL=coupon-service.js.map