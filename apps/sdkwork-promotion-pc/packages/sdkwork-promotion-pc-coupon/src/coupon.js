import { toNullableSdkworkPromotionNumber, toSdkworkPromotionNumber, toSdkworkPromotionOptionalString, } from "@sdkwork/promotion-service";
function createSdkworkAppCapabilityManifest(options = {}) {
    return {
        description: options.description ?? "",
        ...(options.host ? { host: options.host } : {}),
        id: options.id ?? "sdkwork-capability",
        packageNames: [...(options.packageNames ?? [])],
        ...(options.theme ? { theme: options.theme } : {}),
        title: options.title ?? "Capability",
    };
}
function normalizeBasePath(basePath) {
    const normalized = (basePath ?? "/coupons").trim();
    if (!normalized || normalized === "/") {
        return "/coupons";
    }
    return normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
}
function normalizeDiscountMultiplier(rate) {
    if (typeof rate !== "number" || !Number.isFinite(rate) || rate <= 0) {
        return null;
    }
    if (rate <= 1) {
        return rate;
    }
    if (rate <= 10) {
        return rate / 10;
    }
    if (rate <= 100) {
        return rate / 100;
    }
    return null;
}
export function normalizeSdkworkCouponType(value) {
    const normalized = (toSdkworkPromotionOptionalString(value) || "").toUpperCase();
    if (normalized === "CASH") {
        return "cash";
    }
    if (normalized === "DISCOUNT") {
        return "discount";
    }
    if (normalized === "GIFT") {
        return "gift";
    }
    if (normalized === "POINTS_EXCHANGE") {
        return "points-exchange";
    }
    return "unknown";
}
export function normalizeSdkworkCouponAcquireType(value) {
    const normalized = (toSdkworkPromotionOptionalString(value) || "").toUpperCase();
    if (normalized === "RECEIVE") {
        return "receive";
    }
    if (normalized === "REDEEM_CODE") {
        return "redeem-code";
    }
    if (normalized === "POINTS_EXCHANGE") {
        return "points-exchange";
    }
    if (normalized === "ADMIN_GRANT") {
        return "admin-grant";
    }
    return "unknown";
}
export function normalizeSdkworkCouponStatus(value, available) {
    const normalized = (toSdkworkPromotionOptionalString(value) || "").toUpperCase();
    if (available === true
        || normalized === "UNUSED"
        || normalized === "AVAILABLE"
        || normalized === "ACTIVE") {
        return "available";
    }
    if (normalized === "USED") {
        return "used";
    }
    if (normalized === "EXPIRED") {
        return "expired";
    }
    return "inactive";
}
export function normalizeSdkworkRemoteCoupon(coupon, index = 0) {
    const couponId = toSdkworkPromotionOptionalString(coupon.couponId);
    return {
        amountCny: toNullableSdkworkPromotionNumber(coupon.amount),
        canReceive: coupon.canReceive !== false && normalizeSdkworkCouponStatus(coupon.status, coupon.canReceive) === "available",
        couponId,
        description: toSdkworkPromotionOptionalString(coupon.description),
        discountRate: toNullableSdkworkPromotionNumber(coupon.discount),
        endTime: toSdkworkPromotionOptionalString(coupon.endTime),
        getLimit: toNullableSdkworkPromotionNumber(coupon.getLimit),
        id: `coupon-${couponId || index + 1}`,
        minimumSpendCny: toNullableSdkworkPromotionNumber(coupon.minConsume),
        name: toSdkworkPromotionOptionalString(coupon.name) || "Coupon",
        pointCost: toNullableSdkworkPromotionNumber(coupon.pointCost),
        pointsExchange: coupon.pointsExchange === true,
        receivedCount: toNullableSdkworkPromotionNumber(coupon.receivedCount),
        remainingCount: toNullableSdkworkPromotionNumber(coupon.remainingCount),
        scopeType: toSdkworkPromotionOptionalString(coupon.scopeType),
        scopeValue: toSdkworkPromotionOptionalString(coupon.scopeValue),
        stackable: coupon.stackable === true,
        startTime: toSdkworkPromotionOptionalString(coupon.startTime),
        status: normalizeSdkworkCouponStatus(coupon.status, coupon.canReceive),
        statusLabel: toSdkworkPromotionOptionalString(coupon.statusName),
        total: toNullableSdkworkPromotionNumber(coupon.total),
        type: normalizeSdkworkCouponType(coupon.type),
        typeLabel: toSdkworkPromotionOptionalString(coupon.typeName),
        usedCount: toNullableSdkworkPromotionNumber(coupon.usedCount),
    };
}
export function normalizeSdkworkRemoteUserCoupon(coupon, index = 0) {
    const couponId = toSdkworkPromotionOptionalString(coupon.couponId);
    const userCouponId = toSdkworkPromotionOptionalString(coupon.userCouponId);
    return {
        acquireAt: toSdkworkPromotionOptionalString(coupon.acquireAt),
        acquireType: normalizeSdkworkCouponAcquireType(coupon.acquireType),
        amountCny: toNullableSdkworkPromotionNumber(coupon.amount),
        available: coupon.available === true || normalizeSdkworkCouponStatus(coupon.status, coupon.available) === "available",
        code: toSdkworkPromotionOptionalString(coupon.couponCode),
        couponId,
        discountRate: toNullableSdkworkPromotionNumber(coupon.discount),
        expireAt: toSdkworkPromotionOptionalString(coupon.expireAt),
        id: `user-coupon-${userCouponId || couponId || index + 1}`,
        minimumSpendCny: toNullableSdkworkPromotionNumber(coupon.minConsume),
        name: toSdkworkPromotionOptionalString(coupon.couponName) || "Coupon",
        orderId: toSdkworkPromotionOptionalString(coupon.orderId),
        pointCost: toNullableSdkworkPromotionNumber(coupon.pointCost),
        pointsRefundAt: toSdkworkPromotionOptionalString(coupon.pointsRefundAt),
        pointsRefunded: coupon.pointsRefunded === true,
        remainingDays: toNullableSdkworkPromotionNumber(coupon.remainingDays),
        scopeType: toSdkworkPromotionOptionalString(coupon.scopeType),
        scopeValue: toSdkworkPromotionOptionalString(coupon.scopeValue),
        status: normalizeSdkworkCouponStatus(coupon.status, coupon.available),
        statusLabel: toSdkworkPromotionOptionalString(coupon.statusName),
        type: normalizeSdkworkCouponType(coupon.type),
        typeLabel: toSdkworkPromotionOptionalString(coupon.typeName),
        useAt: toSdkworkPromotionOptionalString(coupon.useAt),
        userCouponId,
    };
}
export function sortSdkworkCouponCatalog(coupons) {
    return [...coupons].sort((left, right) => Number(right.canReceive) - Number(left.canReceive)
        || Number(right.pointsExchange) - Number(left.pointsExchange)
        || toSdkworkPromotionNumber(right.amountCny) - toSdkworkPromotionNumber(left.amountCny)
        || toSdkworkPromotionNumber(right.discountRate) - toSdkworkPromotionNumber(left.discountRate)
        || toSdkworkPromotionNumber(left.pointCost ?? Number.MAX_SAFE_INTEGER) - toSdkworkPromotionNumber(right.pointCost ?? Number.MAX_SAFE_INTEGER)
        || left.name.localeCompare(right.name));
}
export function sortSdkworkUserCoupons(coupons) {
    return [...coupons].sort((left, right) => Number(right.status === "available") - Number(left.status === "available")
        || toSdkworkPromotionNumber(right.amountCny) - toSdkworkPromotionNumber(left.amountCny)
        || toSdkworkPromotionNumber(right.discountRate) - toSdkworkPromotionNumber(left.discountRate)
        || toSdkworkPromotionNumber(left.remainingDays ?? Number.MAX_SAFE_INTEGER) - toSdkworkPromotionNumber(right.remainingDays ?? Number.MAX_SAFE_INTEGER)
        || left.name.localeCompare(right.name));
}
export function estimateSdkworkCouponDiscountAmount(amountCny, coupon) {
    if (!coupon || coupon.status === "expired" || coupon.status === "inactive" || coupon.status === "used") {
        return 0;
    }
    const priceCny = Math.max(toSdkworkPromotionNumber(amountCny), 0);
    if (priceCny <= 0) {
        return 0;
    }
    const minimumSpendCny = coupon.minimumSpendCny ?? null;
    if (minimumSpendCny !== null && priceCny < minimumSpendCny) {
        return 0;
    }
    const fixedDiscount = Math.max(toSdkworkPromotionNumber(coupon.discountAmountCny), 0);
    if (fixedDiscount > 0) {
        return Math.min(fixedDiscount, priceCny);
    }
    const discountMultiplier = normalizeDiscountMultiplier(coupon.discountRate ?? null);
    if (discountMultiplier === null) {
        return 0;
    }
    return Math.max(0, Math.min(priceCny, Math.round((priceCny * (1 - discountMultiplier)) * 100) / 100));
}
export function summarizeSdkworkCouponCatalog(coupons) {
    return coupons.reduce((summary, coupon) => {
        summary.totalCoupons += 1;
        if (coupon.canReceive && coupon.status === "available") {
            summary.claimableCoupons += 1;
        }
        if (coupon.pointsExchange) {
            summary.pointsExchangeCoupons += 1;
        }
        return summary;
    }, {
        claimableCoupons: 0,
        pointsExchangeCoupons: 0,
        totalCoupons: 0,
    });
}
export function summarizeSdkworkUserCoupons(coupons) {
    return coupons.reduce((summary, coupon) => {
        summary.totalCoupons += 1;
        summary.highestDiscountAmountCny = Math.max(summary.highestDiscountAmountCny, toSdkworkPromotionNumber(coupon.discountAmountCny));
        if (coupon.status === "available") {
            summary.availableCoupons += 1;
            if ((coupon.remainingDays ?? null) !== null && (coupon.remainingDays ?? Number.MAX_SAFE_INTEGER) <= 7) {
                summary.expiringSoonCoupons += 1;
            }
        }
        return summary;
    }, {
        availableCoupons: 0,
        expiringSoonCoupons: 0,
        highestDiscountAmountCny: 0,
        totalCoupons: 0,
    });
}
export function resolveSdkworkUserCouponRequestId(coupon) {
    if (coupon.userCouponId) {
        return coupon.userCouponId;
    }
    if (coupon.couponId) {
        return coupon.couponId;
    }
    if (coupon.id.startsWith("user-coupon-")) {
        return coupon.id.slice("user-coupon-".length);
    }
    return undefined;
}
export function createCouponWorkspaceManifest({ description = "Coupon workspace for discovery, redemption, points exchange, and reusable checkout discount operations.", host, id = "sdkwork-coupon", packageNames = ["@sdkwork/promotion-pc-coupon"], routePath = "/coupons", theme, title = "Coupons", } = {}) {
    return {
        ...createSdkworkAppCapabilityManifest({
            description,
            host,
            id,
            packageNames,
            theme,
            title,
        }),
        capability: "coupon",
        routePath: normalizeBasePath(routePath),
    };
}
export function createCouponRouteIntent(options = {}) {
    const basePath = normalizeBasePath(options.basePath);
    const queryParams = new URLSearchParams();
    if (options.tab) {
        queryParams.set("tab", options.tab);
    }
    if (options.couponId) {
        queryParams.set("couponId", options.couponId);
    }
    if (options.userCouponId) {
        queryParams.set("userCouponId", options.userCouponId);
    }
    const querySuffix = queryParams.toString() ? `?${queryParams.toString()}` : "";
    return {
        ...(options.couponId ? { couponId: options.couponId } : {}),
        focusWindow: options.focusWindow !== false,
        route: `${basePath}${querySuffix}`,
        source: "coupon-workspace",
        ...(options.tab ? { tab: options.tab } : {}),
        type: "coupon-route-intent",
        ...(options.userCouponId ? { userCouponId: options.userCouponId } : {}),
    };
}
export const couponPackageMeta = {
    architecture: "pc-react",
    domain: "commerce",
    package: "@sdkwork/promotion-pc-coupon",
    status: "ready",
};
//# sourceMappingURL=coupon.js.map