import type { SdkworkCouponMessagesOverrides } from "../coupon-copy";
import type { SdkworkCouponController } from "../coupon-controller";
export interface SdkworkCouponPageProps {
    controller?: SdkworkCouponController;
    locale?: string | null;
    messages?: SdkworkCouponMessagesOverrides;
}
export declare function SdkworkCouponPage({ locale, messages, ...props }: SdkworkCouponPageProps): import("react").JSX.Element;
//# sourceMappingURL=CouponPage.d.ts.map