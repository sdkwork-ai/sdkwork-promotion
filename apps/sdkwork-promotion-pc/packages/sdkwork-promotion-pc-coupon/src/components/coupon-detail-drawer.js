import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { Button, DetailDrawer, DetailDrawerMetric, DetailDrawerMetrics, DetailDrawerSection, } from "@sdkwork/ui-pc-react";
import { useSdkworkCouponControllerState } from "../coupon-controller";
import { createSdkworkCouponMetricToneStyle, resolveSdkworkCouponStatusTone, } from "../coupon-appearance";
import { useSdkworkCouponIntl } from "../coupon-intl";
export function SdkworkCouponDetailDrawer({ controller, }) {
    const state = useSdkworkCouponControllerState(controller);
    const { copy, formatAvailability, formatCurrencyCny, formatPointCost, formatRemainingDays, formatStatus, formatTimestamp, formatType, } = useSdkworkCouponIntl();
    const detail = state.detail;
    const catalogDetail = state.detailKind === "catalog" && detail ? detail : null;
    const ownedDetail = state.detailKind === "owned" && detail ? detail : null;
    const emptyValue = copy.common.emptyValue;
    const expireAtLabel = ownedDetail
        ? formatTimestamp(ownedDetail.expireAt)
        : catalogDetail
            ? formatTimestamp(catalogDetail.endTime)
            : emptyValue;
    const acquiredAtLabel = ownedDetail ? formatTimestamp(ownedDetail.acquireAt) : emptyValue;
    const stackableLabel = catalogDetail ? formatAvailability(catalogDetail.stackable) : emptyValue;
    const pointCostLabel = detail ? formatPointCost(detail.pointCost) : emptyValue;
    const availabilityLabel = ownedDetail
        ? formatAvailability(ownedDetail.available)
        : catalogDetail
            ? formatAvailability(catalogDetail.canReceive)
            : emptyValue;
    const remainingDaysLabel = ownedDetail ? formatRemainingDays(ownedDetail.remainingDays) : emptyValue;
    const orderIdLabel = ownedDetail?.orderId || emptyValue;
    const statusTone = detail ? resolveSdkworkCouponStatusTone(detail.status) : "default";
    const useAtLabel = ownedDetail ? formatTimestamp(ownedDetail.useAt) : emptyValue;
    return (_jsx(DetailDrawer, { description: detail?.name || copy.detail.summaryFallback, eyebrow: catalogDetail ? copy.detail.catalogEyebrow : ownedDetail ? copy.detail.ownedEyebrow : copy.detail.title, footer: (_jsxs("div", { className: "flex flex-wrap justify-end gap-3", children: [catalogDetail?.canReceive ? (_jsx(Button, { onClick: () => void controller.receiveCoupon(catalogDetail.id).catch(() => { }), type: "button", children: copy.actions.claimCoupon })) : null, catalogDetail?.pointsExchange ? (_jsx(Button, { onClick: () => void controller.exchangeCouponByPoints({ couponId: catalogDetail.id }).catch(() => { }), type: "button", variant: "outline", children: copy.actions.exchangePoints })) : null, ownedDetail?.status === "used" ? (_jsx(Button, { onClick: () => void controller.cancelUseCoupon(ownedDetail.userCouponId ?? ownedDetail.id).catch(() => { }), type: "button", variant: "outline", children: copy.actions.cancelUse })) : null, ownedDetail && (ownedDetail.pointCost ?? 0) > 0 && !ownedDetail.pointsRefunded ? (_jsx(Button, { onClick: () => void controller.rollbackPointsExchange({ userCouponId: ownedDetail.userCouponId }).catch(() => { }), type: "button", variant: "outline", children: copy.actions.rollbackPoints })) : null, _jsx(Button, { onClick: () => controller.closeDetail(), type: "button", variant: "ghost", children: copy.actions.close })] })), onOpenChange: (open) => {
            if (!open) {
                controller.closeDetail();
            }
        }, open: state.isDetailOpen, summary: detail ? (_jsxs("div", { className: "flex flex-wrap items-center gap-3", children: [_jsx("span", { className: "rounded-full border px-3 py-1 text-xs font-semibold uppercase tracking-[0.14em]", "data-sdk-tone": statusTone, style: createSdkworkCouponMetricToneStyle(statusTone), children: formatStatus(detail.status) }), _jsx("span", { className: "font-medium text-[var(--sdk-color-text-primary)]", children: formatCurrencyCny(detail.amountCny) }), _jsx("span", { children: pointCostLabel })] })) : copy.detail.loading, title: copy.detail.title, children: state.isDetailLoading || !detail ? (_jsx("div", { className: "text-sm text-[var(--sdk-color-text-secondary)]", children: copy.detail.loading })) : (_jsxs(_Fragment, { children: [_jsxs(DetailDrawerMetrics, { columns: 3, children: [_jsx(DetailDrawerMetric, { label: copy.common.coupon, value: detail.name }), _jsx(DetailDrawerMetric, { label: copy.detail.statusMetricLabel, tone: statusTone, value: formatStatus(detail.status) }), _jsx(DetailDrawerMetric, { label: copy.detail.discountMetricLabel, value: formatCurrencyCny(detail.amountCny) }), _jsx(DetailDrawerMetric, { label: copy.detail.pointCostLabel, value: pointCostLabel })] }), _jsx(DetailDrawerSection, { description: copy.detail.overviewDescription, title: copy.detail.overviewTitle, children: _jsxs("div", { className: "grid gap-3 text-sm text-[var(--sdk-color-text-secondary)] sm:grid-cols-2", children: [_jsxs("div", { children: [copy.detail.typeLabel, ": ", formatType(detail.type)] }), _jsxs("div", { children: [copy.detail.minimumSpendLabel, ": ", formatCurrencyCny(detail.minimumSpendCny)] }), _jsxs("div", { children: [copy.detail.availabilityLabel, ": ", availabilityLabel] }), _jsxs("div", { children: [copy.detail.remainingDaysLabel, ": ", remainingDaysLabel] }), _jsxs("div", { children: [copy.detail.expireAtLabel, ": ", expireAtLabel] }), _jsxs("div", { children: [copy.detail.acquiredAtLabel, ": ", acquiredAtLabel] })] }) }), _jsx(DetailDrawerSection, { description: copy.detail.usageDescription, title: copy.detail.usageTitle, children: _jsxs("div", { className: "grid gap-3 text-sm text-[var(--sdk-color-text-secondary)] sm:grid-cols-2", children: [_jsxs("div", { children: [copy.detail.stackableLabel, ": ", stackableLabel] }), _jsxs("div", { children: [copy.detail.orderIdLabel, ": ", orderIdLabel] }), _jsxs("div", { children: [copy.detail.useAtLabel, ": ", useAtLabel] }), _jsxs("div", { children: [copy.detail.couponIdLabel, ": ", "couponId" in detail ? detail.couponId || detail.id : detail.id] }), _jsxs("div", { children: [copy.detail.userCouponIdLabel, ": ", "userCouponId" in detail ? detail.userCouponId || emptyValue : emptyValue] })] }) })] })) }));
}
//# sourceMappingURL=coupon-detail-drawer.js.map