import { type PropsWithChildren } from "react";
import { type SdkworkCouponMessages, type SdkworkCouponMessagesOverrides } from "./coupon-copy";
export interface SdkworkCouponIntlValue {
    copy: SdkworkCouponMessages;
    formatAvailability: (value: boolean | null | undefined) => string;
    formatCurrencyCny: (value: number | null | undefined) => string;
    formatPointCost: (value: number | null | undefined) => string;
    formatRemainingDays: (value: number | null | undefined) => string;
    formatStatus: (status: string | null | undefined) => string;
    formatTimestamp: (value: string | undefined) => string;
    formatType: (value: string | null | undefined) => string;
    locale: string;
}
export interface SdkworkCouponIntlProviderProps extends PropsWithChildren {
    locale?: string | null;
    messages?: SdkworkCouponMessagesOverrides;
}
export declare function SdkworkCouponIntlProvider({ children, locale, messages, }: SdkworkCouponIntlProviderProps): import("react").JSX.Element;
export declare function useSdkworkCouponIntl(): SdkworkCouponIntlValue;
//# sourceMappingURL=coupon-intl.d.ts.map