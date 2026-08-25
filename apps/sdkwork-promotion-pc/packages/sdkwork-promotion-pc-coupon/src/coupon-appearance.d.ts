import type { CSSProperties } from "react";
import { type SdkworkThemeVisualTone } from "@sdkwork/ui-pc-react/theme";
export type SdkworkCouponVisualTone = SdkworkThemeVisualTone;
export type SdkworkCouponMetricTone = "default" | "danger" | "success" | "warning";
export declare function resolveSdkworkCouponStatusTone(status: string | null | undefined): SdkworkCouponMetricTone;
export declare function createSdkworkCouponToneStyle(tone: SdkworkCouponVisualTone, options?: {
    backgroundWeight?: number;
    borderWeight?: number;
}): CSSProperties;
export declare function createSdkworkCouponMetricToneStyle(tone: SdkworkCouponMetricTone): CSSProperties;
export declare function createSdkworkCouponPanelStyle(tone: SdkworkCouponVisualTone, options?: {
    backgroundWeight?: number;
    borderWeight?: number;
    surfaceColor?: string;
    surfaceWeight?: number;
}): CSSProperties;
export declare function createSdkworkCouponGlassStyle(tone: SdkworkCouponVisualTone, options?: {
    backgroundWeight?: number;
    borderWeight?: number;
    surfaceColor?: string;
    surfaceWeight?: number;
}): CSSProperties;
export declare function createSdkworkCouponBackdropStyle(): CSSProperties;
export declare function createSdkworkCouponHeroStyle(): CSSProperties;
export declare function createSdkworkCouponHeroTextStyle(tone?: "muted" | "primary" | "subtle"): CSSProperties;
//# sourceMappingURL=coupon-appearance.d.ts.map