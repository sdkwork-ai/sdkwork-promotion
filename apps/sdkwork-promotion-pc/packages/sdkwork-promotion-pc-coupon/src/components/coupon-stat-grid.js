import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { createSdkworkCouponPanelStyle } from "../coupon-appearance";
import { useSdkworkCouponIntl } from "../coupon-intl";
export function SdkworkCouponStatGrid({ catalogDigest, statistics, userDigest, }) {
    const { copy, formatCurrencyCny, } = useSdkworkCouponIntl();
    const cards = [
        {
            label: copy.stats.availableCoupons,
            value: userDigest.availableCoupons,
        },
        {
            label: copy.stats.claimableOffers,
            value: catalogDigest.claimableCoupons,
        },
        {
            label: copy.stats.expiringSoon,
            value: userDigest.expiringSoonCoupons,
        },
        {
            label: copy.stats.highestDiscount,
            value: formatCurrencyCny(userDigest.highestDiscountAmountCny),
        },
        {
            label: copy.stats.usedCoupons,
            value: statistics.usedCount,
        },
        {
            label: copy.stats.totalInventory,
            value: statistics.totalCoupons,
        },
    ];
    return (_jsx("div", { className: "grid gap-4 md:grid-cols-2 xl:grid-cols-3", children: cards.map((card) => (_jsxs("article", { className: "rounded-[1.6rem] border px-5 py-5 shadow-[var(--sdk-shadow-md)]", style: createSdkworkCouponPanelStyle("neutral", {
                backgroundWeight: 8,
                borderWeight: 18,
            }), children: [_jsx("div", { className: "text-[0.7rem] font-semibold uppercase tracking-[0.18em] text-[var(--sdk-color-text-muted)]", children: card.label }), _jsx("div", { className: "mt-3 text-3xl font-semibold tracking-tight text-[var(--sdk-color-text-primary)]", children: card.value })] }, card.label))) }));
}
//# sourceMappingURL=coupon-stat-grid.js.map