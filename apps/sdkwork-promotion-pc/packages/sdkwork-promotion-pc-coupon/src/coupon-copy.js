function isRecord(value) {
    return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
function mergeDeep(base, overrides) {
    if (!overrides) {
        return base;
    }
    const output = {
        ...base,
    };
    for (const [key, value] of Object.entries(overrides)) {
        if (value === undefined) {
            continue;
        }
        const baseValue = output[key];
        output[key] = isRecord(baseValue) && isRecord(value)
            ? mergeDeep(baseValue, value)
            : value;
    }
    return output;
}
const EN_US_MESSAGES = {
    actions: {
        cancelUse: "Cancel use",
        claimCoupon: "Claim coupon",
        close: "Close",
        discover: "Discover",
        exchangePoints: "Exchange points",
        history: "History",
        myCoupons: "My coupons",
        redeemCode: "Redeem coupon",
        refreshInventory: "Refresh inventory",
        rollbackPoints: "Restore points",
        viewDetails: "View details",
    },
    common: {
        available: "Available",
        coupon: "Coupon",
        days: "days",
        emptyValue: "--",
        no: "No",
        pointCost: "point cost",
        yes: "Yes",
    },
    controller: {
        bootstrapFailed: "Failed to load coupon center.",
        cancelUseFailed: "Failed to cancel coupon usage.",
        couponDetailFailed: "Failed to load coupon detail.",
        exchangeFailed: "Failed to exchange coupon by points.",
        receiveFailed: "Failed to receive coupon.",
        redeemFailed: "Failed to redeem coupon.",
        rollbackFailed: "Failed to rollback coupon points exchange.",
        selectCouponRequired: "Select a coupon before continuing.",
        useFailed: "Failed to use coupon.",
        userCouponDetailFailed: "Failed to load user coupon detail.",
    },
    detail: {
        acquiredAtLabel: "Acquired at",
        availabilityLabel: "Available",
        catalogEyebrow: "Catalog offer",
        couponIdLabel: "Coupon id",
        discountMetricLabel: "Discount",
        expireAtLabel: "Expire at",
        loading: "Loading coupon detail...",
        minimumSpendLabel: "Minimum spend",
        ownedEyebrow: "Your coupon",
        orderIdLabel: "Order id",
        overviewDescription: "Review coupon basics, availability, and the time window that matters most.",
        overviewTitle: "Overview",
        pointCostLabel: "Point cost",
        remainingDaysLabel: "Remaining days",
        stackableLabel: "Stackable",
        statusMetricLabel: "Status",
        summaryFallback: "Review savings rules, availability, and next actions.",
        title: "Coupon detail",
        typeLabel: "Type",
        usageDescription: "Check recovery actions, checkout readiness, and lifecycle traceability.",
        usageTitle: "Usage",
        useAtLabel: "Use at",
        userCouponIdLabel: "User coupon id",
    },
    page: {
        activeCouponFallback: "No active coupon",
        claimableOffersLabel: "Claimable offers",
        description: "Keep every reusable offer, coupon code, and checkout discount in one Sdkwork-aligned savings workspace.",
        errorTitle: "Coupon center error",
        eyebrow: "Savings system",
        highestDiscountLabel: "Highest discount",
        inventoryEyebrow: "Inventory",
        inventoryTitle: "Coupon inventory",
        loading: "Loading coupon center...",
        title: "Coupon center",
    },
    format: {
        pointCostValue: "{value} points",
        remainingDaysValue: "{value} days",
    },
    inventory: {
        catalogFallbackDescription: "Reusable savings offer",
        codeLabel: "Code",
        emptyDiscover: "No discoverable coupons are currently available.",
        emptyVisible: "No coupons are visible in this view.",
        pointCostLabel: "Point cost",
        remainingDaysLabel: "Remaining days",
    },
    redeemDialog: {
        benefitCheckout: "Use new offers across subscription, payment, and wallet flows without re-entering codes.",
        benefitInventory: "New coupon codes land in the same inventory as your claimed and exchanged offers.",
        benefitRecovery: "Restore trackable discount operations before they drift out of sync with checkout.",
        description: "Redeem a coupon code and add the offer to your ready-to-use inventory.",
        eyebrow: "Redeem flow",
        errorTitle: "Coupon redeem error",
        inputLabel: "Coupon code",
        inputPlaceholder: "Enter your coupon code",
        previewLabel: "Preview",
        summaryDescription: "Check the code, confirm the flow, and keep the coupon ready for the next premium checkout.",
        summaryTitle: "One code, reusable everywhere",
        title: "Redeem coupon",
    },
    service: {
        cancelUseFailed: "Failed to cancel coupon usage.",
        couponDetailFailed: "Failed to load coupon detail.",
        exchangeFailed: "Failed to exchange coupon by points.",
        receiveFailed: "Failed to receive coupon.",
        redeemFailed: "Failed to redeem coupon.",
        requestFailed: "Request failed.",
        rollbackFailed: "Failed to rollback coupon points exchange.",
        signInRequired: "Please sign in to manage coupons.",
        useFailed: "Failed to use coupon.",
        userCouponDetailFailed: "Failed to load user coupon detail.",
    },
    stats: {
        availableCoupons: "Available coupons",
        claimableOffers: "Claimable offers",
        expiringSoon: "Expiring soon",
        highestDiscount: "Highest discount",
        pointsExchangeOffers: "Points exchange offers",
        totalInventory: "Total inventory",
        usedCoupons: "Used coupons",
    },
    status: {
        available: "Available",
        expired: "Expired",
        inactive: "Inactive",
        used: "Used",
    },
    type: {
        cash: "Cash",
        discount: "Discount",
        gift: "Gift",
        pointsExchange: "Points exchange",
        unknown: "Unknown",
    },
};
const ZH_CN_MESSAGES = {
    actions: {
        cancelUse: "取消使用",
        claimCoupon: "领取优惠券",
        close: "关闭",
        discover: "发现",
        exchangePoints: "积分兑换",
        history: "历史",
        myCoupons: "我的优惠券",
        redeemCode: "兑换优惠券",
        refreshInventory: "刷新库存",
        rollbackPoints: "退回积分",
        viewDetails: "查看详情",
    },
    common: {
        available: "可用",
        coupon: "优惠券",
        days: "天",
        emptyValue: "--",
        no: "否",
        pointCost: "积分成本",
        yes: "是",
    },
    controller: {
        bootstrapFailed: "加载优惠券中心失败。",
        cancelUseFailed: "取消使用优惠券失败。",
        couponDetailFailed: "加载优惠券详情失败。",
        exchangeFailed: "积分兑换优惠券失败。",
        receiveFailed: "领取优惠券失败。",
        redeemFailed: "兑换优惠券失败。",
        rollbackFailed: "回退积分兑换失败。",
        selectCouponRequired: "请先选择优惠券。",
        useFailed: "使用优惠券失败。",
        userCouponDetailFailed: "加载用户优惠券详情失败。",
    },
    detail: {
        acquiredAtLabel: "领取时间",
        availabilityLabel: "是否可用",
        catalogEyebrow: "券库方案",
        couponIdLabel: "优惠券编号",
        discountMetricLabel: "优惠额度",
        expireAtLabel: "过期时间",
        loading: "正在加载优惠券详情...",
        minimumSpendLabel: "最低消费",
        ownedEyebrow: "我的优惠券",
        orderIdLabel: "订单编号",
        overviewDescription: "优先查看优惠基础信息、可用状态和最关键的时间窗口。",
        overviewTitle: "概览",
        pointCostLabel: "积分成本",
        remainingDaysLabel: "剩余天数",
        stackableLabel: "是否可叠加",
        statusMetricLabel: "状态",
        summaryFallback: "查看优惠规则、可用状态与下一步动作。",
        title: "优惠券详情",
        typeLabel: "类型",
        usageDescription: "查看恢复动作、结算准备状态与生命周期轨迹。",
        usageTitle: "使用信息",
        useAtLabel: "使用时间",
        userCouponIdLabel: "用户优惠券编号",
    },
    page: {
        activeCouponFallback: "当前没有激活优惠券",
        claimableOffersLabel: "可领取优惠",
        description: "把可复用优惠、券码兑换和结算折扣放到同一套 Sdkwork 风格的优惠工作台里统一管理。",
        errorTitle: "优惠券中心异常",
        eyebrow: "优惠体系",
        highestDiscountLabel: "最高优惠",
        inventoryEyebrow: "库存工作台",
        inventoryTitle: "优惠券库存",
        loading: "正在加载优惠券中心...",
        title: "优惠券中心",
    },
    format: {
        pointCostValue: "{value} 积分",
        remainingDaysValue: "{value} 天",
    },
    inventory: {
        catalogFallbackDescription: "可复用优惠方案",
        codeLabel: "券码",
        emptyDiscover: "当前没有可发现的优惠券。",
        emptyVisible: "当前视图下没有可展示的优惠券。",
        pointCostLabel: "积分成本",
        remainingDaysLabel: "剩余天数",
    },
    redeemDialog: {
        benefitCheckout: "新的优惠可以直接复用于订阅、支付和钱包相关流程，不必重复录入。",
        benefitInventory: "券码兑换后的优惠会和领取、积分兑换的优惠一起沉淀到同一库存里。",
        benefitRecovery: "在优惠参与结算后，也能继续追踪恢复动作与状态变化。",
        description: "兑换券码后，优惠会立刻加入你的待用库存。",
        eyebrow: "券码兑换",
        errorTitle: "兑换优惠券异常",
        inputLabel: "券码",
        inputPlaceholder: "输入券码",
        previewLabel: "即时预览",
        summaryDescription: "先确认券码，再完成兑换，让新的优惠直接进入下一次高级结算流程。",
        summaryTitle: "一张券码，多处复用",
        title: "兑换优惠券",
    },
    service: {
        cancelUseFailed: "取消使用优惠券失败。",
        couponDetailFailed: "加载优惠券详情失败。",
        exchangeFailed: "积分兑换优惠券失败。",
        receiveFailed: "领取优惠券失败。",
        redeemFailed: "兑换优惠券失败。",
        requestFailed: "请求失败。",
        rollbackFailed: "回退积分兑换失败。",
        signInRequired: "请先登录后再管理优惠券。",
        useFailed: "使用优惠券失败。",
        userCouponDetailFailed: "加载用户优惠券详情失败。",
    },
    stats: {
        availableCoupons: "可用优惠券",
        claimableOffers: "可领取优惠",
        expiringSoon: "即将过期",
        highestDiscount: "最高优惠",
        pointsExchangeOffers: "积分兑换优惠",
        totalInventory: "总库存",
        usedCoupons: "已使用优惠券",
    },
    status: {
        available: "可用",
        expired: "已过期",
        inactive: "未激活",
        used: "已使用",
    },
    type: {
        cash: "满减券",
        discount: "折扣券",
        gift: "赠品券",
        pointsExchange: "积分兑换券",
        unknown: "未知类型",
    },
};
const SDKWORK_COUPON_MESSAGES = {
    "en-US": EN_US_MESSAGES,
    "zh-CN": ZH_CN_MESSAGES,
};
export function normalizeSdkworkCouponLocale(locale) {
    const normalized = String(locale || "").trim().toLowerCase();
    if (normalized.startsWith("zh")) {
        return "zh-CN";
    }
    return "en-US";
}
export function createSdkworkCouponMessages(locale, overrides) {
    return mergeDeep(SDKWORK_COUPON_MESSAGES[normalizeSdkworkCouponLocale(locale)], overrides);
}
//# sourceMappingURL=coupon-copy.js.map