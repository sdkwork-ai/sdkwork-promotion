import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useEffect, useState } from "react";
import { Button, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, Input, StatusNotice, } from "@sdkwork/ui-pc-react";
import { useSdkworkCouponControllerState } from "../coupon-controller";
import { useSdkworkCouponIntl } from "../coupon-intl";
export function SdkworkCouponRedeemDialog({ controller, }) {
    const state = useSdkworkCouponControllerState(controller);
    const { copy } = useSdkworkCouponIntl();
    const [redeemCode, setRedeemCode] = useState("");
    useEffect(() => {
        if (state.isRedeemOpen) {
            setRedeemCode("");
        }
    }, [state.isRedeemOpen]);
    return (_jsx(Dialog, { onOpenChange: (open) => {
            if (!open) {
                controller.closeRedeemDialog();
            }
        }, open: state.isRedeemOpen, children: _jsxs(DialogContent, { className: "w-[min(92vw,28rem)] gap-0 overflow-hidden p-0", children: [_jsxs(DialogHeader, { className: "border-b border-[var(--sdk-color-border-subtle)] px-6 py-5", children: [_jsx(DialogTitle, { children: copy.redeemDialog.title }), _jsx(DialogDescription, { children: copy.redeemDialog.summaryDescription })] }), _jsxs("div", { className: "space-y-4 px-6 py-5", children: [state.lastError ? (_jsx(StatusNotice, { title: copy.redeemDialog.errorTitle, tone: "danger", children: state.lastError })) : null, _jsxs("form", { className: "space-y-4", onSubmit: (event) => {
                                event.preventDefault();
                                void controller.redeemCoupon({
                                    redeemCode: redeemCode.trim(),
                                }).catch(() => { });
                            }, children: [_jsxs("label", { className: "block space-y-2", htmlFor: "sdkwork-coupon-redeem-code", children: [_jsx("span", { className: "text-sm font-medium text-[var(--sdk-color-text-primary)]", children: copy.redeemDialog.inputLabel }), _jsx(Input, { id: "sdkwork-coupon-redeem-code", onChange: (event) => setRedeemCode(event.target.value), placeholder: copy.redeemDialog.inputPlaceholder, required: true, value: redeemCode })] }), _jsxs(DialogFooter, { className: "gap-2 sm:justify-end", children: [_jsx(Button, { onClick: () => controller.closeRedeemDialog(), type: "button", variant: "ghost", children: copy.actions.close }), _jsx(Button, { disabled: !redeemCode.trim(), loading: state.isMutating, type: "submit", children: copy.actions.redeemCode })] })] })] })] }) }));
}
//# sourceMappingURL=coupon-redeem-dialog.js.map