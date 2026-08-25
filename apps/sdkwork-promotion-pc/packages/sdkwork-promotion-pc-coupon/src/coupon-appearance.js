import { createSdkworkBackdropStyle, createSdkworkGlassStyle, createSdkworkHeroStyle, createSdkworkPanelStyle, createSdkworkToneStyle } from "@sdkwork/ui-pc-react/theme";
export function resolveSdkworkCouponStatusTone(status) {
    const normalized = String(status || "").trim().toLowerCase();
    if (normalized === "available") {
        return "success";
    }
    if (normalized === "used") {
        return "warning";
    }
    if (normalized === "expired" || normalized === "inactive") {
        return "danger";
    }
    return "default";
}
export function createSdkworkCouponToneStyle(tone, options = {}) {
    return createSdkworkToneStyle(tone, options);
}
export function createSdkworkCouponMetricToneStyle(tone) {
    if (tone === "success") {
        return createSdkworkCouponToneStyle("success", {
            backgroundWeight: 14,
            borderWeight: 26,
        });
    }
    if (tone === "warning") {
        return createSdkworkCouponToneStyle("warning", {
            backgroundWeight: 14,
            borderWeight: 26,
        });
    }
    if (tone === "danger") {
        return createSdkworkCouponToneStyle("danger", {
            backgroundWeight: 14,
            borderWeight: 26,
        });
    }
    return createSdkworkCouponToneStyle("neutral", {
        backgroundWeight: 10,
        borderWeight: 22,
    });
}
export function createSdkworkCouponPanelStyle(tone, options = {}) {
    return createSdkworkPanelStyle(tone, options);
}
export function createSdkworkCouponGlassStyle(tone, options = {}) {
    return createSdkworkGlassStyle(tone, options);
}
export function createSdkworkCouponBackdropStyle() {
    return createSdkworkBackdropStyle();
}
export function createSdkworkCouponHeroStyle() {
    return createSdkworkHeroStyle();
}
export function createSdkworkCouponHeroTextStyle(tone = "primary") {
    if (tone === "muted") {
        return {
            color: "color-mix(in srgb, white 72%, var(--sdk-color-brand-accent))",
        };
    }
    if (tone === "subtle") {
        return {
            color: "color-mix(in srgb, white 64%, var(--sdk-color-brand-accent))",
        };
    }
    return {
        color: "color-mix(in srgb, white 92%, var(--sdk-color-brand-accent))",
    };
}
//# sourceMappingURL=coupon-appearance.js.map