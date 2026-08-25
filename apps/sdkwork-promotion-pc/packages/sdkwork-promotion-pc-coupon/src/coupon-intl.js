import { jsx as _jsx } from "react/jsx-runtime";
import { createContext, useContext, useMemo, } from "react";
import { formatSdkworkPromotionCurrencyCny as formatSdkworkCurrencyCny, formatSdkworkPromotionPoints as formatSdkworkPoints, } from "@sdkwork/promotion-service";
import { createSdkworkCouponMessages, normalizeSdkworkCouponLocale, } from "./coupon-copy";
function interpolateSdkworkCouponTemplate(template, values) {
    return Object.entries(values).reduce((output, [key, value]) => output.replaceAll(`{${key}}`, value), template);
}
function createSdkworkCouponIntlValue(locale, overrides) {
    const resolvedLocale = normalizeSdkworkCouponLocale(locale);
    const copy = createSdkworkCouponMessages(resolvedLocale, overrides);
    return {
        copy,
        formatAvailability(value) {
            return value ? copy.common.yes : copy.common.no;
        },
        formatCurrencyCny(value) {
            return formatSdkworkCurrencyCny(value, resolvedLocale);
        },
        formatPointCost(value) {
            if (value === null || value === undefined) {
                return copy.common.emptyValue;
            }
            return interpolateSdkworkCouponTemplate(copy.format.pointCostValue, {
                value: formatSdkworkPoints(value, resolvedLocale),
            });
        },
        formatRemainingDays(value) {
            if (value === null || value === undefined) {
                return copy.common.emptyValue;
            }
            return interpolateSdkworkCouponTemplate(copy.format.remainingDaysValue, {
                value: formatSdkworkPoints(value, resolvedLocale),
            });
        },
        formatStatus(status) {
            const normalized = String(status || "").trim().toLowerCase();
            return copy.status[normalized] ?? status ?? copy.status.inactive;
        },
        formatTimestamp(value) {
            if (!value) {
                return copy.common.emptyValue;
            }
            return new Intl.DateTimeFormat(resolvedLocale, {
                dateStyle: "medium",
                timeStyle: "short",
            }).format(new Date(value));
        },
        formatType(value) {
            const normalized = String(value || "").trim().toLowerCase().replaceAll("-", "");
            if (normalized === "pointsexchange") {
                return copy.type.pointsExchange;
            }
            return copy.type[normalized] ?? copy.type.unknown;
        },
        locale: resolvedLocale,
    };
}
const DEFAULT_SDKWORK_COUPON_INTL = createSdkworkCouponIntlValue();
const SdkworkCouponIntlContext = createContext(DEFAULT_SDKWORK_COUPON_INTL);
export function SdkworkCouponIntlProvider({ children, locale, messages, }) {
    const value = useMemo(() => createSdkworkCouponIntlValue(locale, messages), [locale, messages]);
    return (_jsx(SdkworkCouponIntlContext.Provider, { value: value, children: children }));
}
export function useSdkworkCouponIntl() {
    return useContext(SdkworkCouponIntlContext);
}
//# sourceMappingURL=coupon-intl.js.map