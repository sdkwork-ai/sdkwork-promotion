import { useMemo, useSyncExternalStore, } from "react";
import { createSdkworkCouponService, } from "./coupon-service";
import { createSdkworkCouponMessages, } from "./coupon-copy";
function normalizeDashboard(dashboard, fallbackDashboard) {
    const resolvedDashboard = dashboard ?? fallbackDashboard;
    return {
        ...fallbackDashboard,
        ...resolvedDashboard,
        availableCoupons: [...(resolvedDashboard.availableCoupons ?? fallbackDashboard.availableCoupons ?? [])],
        catalogCoupons: [...(resolvedDashboard.catalogCoupons ?? fallbackDashboard.catalogCoupons ?? [])],
        catalogDigest: resolvedDashboard.catalogDigest ?? fallbackDashboard.catalogDigest,
        myCoupons: [...(resolvedDashboard.myCoupons ?? fallbackDashboard.myCoupons ?? [])],
        statistics: resolvedDashboard.statistics ?? fallbackDashboard.statistics,
        userDigest: resolvedDashboard.userDigest ?? fallbackDashboard.userDigest,
    };
}
function resolveSelectedCatalogCouponId(dashboard, selectedCatalogCouponId) {
    if (selectedCatalogCouponId && dashboard.catalogCoupons.some((coupon) => coupon.id === selectedCatalogCouponId)) {
        return selectedCatalogCouponId;
    }
    return dashboard.catalogCoupons[0]?.id ?? null;
}
function resolveSelectedUserCouponId(dashboard, selectedUserCouponId) {
    if (selectedUserCouponId && dashboard.myCoupons.some((coupon) => coupon.id === selectedUserCouponId)) {
        return selectedUserCouponId;
    }
    return dashboard.availableCoupons[0]?.id
        ?? dashboard.myCoupons[0]?.id
        ?? null;
}
function deriveVisibleUserCoupons(dashboard, activeTab) {
    if (activeTab === "history") {
        return dashboard.myCoupons.filter((coupon) => coupon.status !== "available");
    }
    return dashboard.availableCoupons;
}
function normalizeState(state, fallbackDashboard) {
    const dashboard = normalizeDashboard(state.dashboard, fallbackDashboard);
    return {
        ...state,
        dashboard,
        selectedCatalogCouponId: resolveSelectedCatalogCouponId(dashboard, state.selectedCatalogCouponId),
        selectedUserCouponId: resolveSelectedUserCouponId(dashboard, state.selectedUserCouponId),
        visibleCatalogCoupons: dashboard.catalogCoupons,
        visibleUserCoupons: deriveVisibleUserCoupons(dashboard, state.activeTab),
    };
}
function stripCouponId(value) {
    return value.startsWith("coupon-") ? value.slice("coupon-".length) : value;
}
function stripUserCouponId(value) {
    return value.startsWith("user-coupon-") ? value.slice("user-coupon-".length) : value;
}
export function createSdkworkCouponController(options = {}) {
    const copy = createSdkworkCouponMessages(options.locale, options.messages).controller;
    const fallbackDashboard = (options.service?.getEmptyDashboard
        ?? createSdkworkCouponService({
            locale: options.locale,
            messages: options.messages,
        }).getEmptyDashboard)();
    const service = options.service
        ? {
            ...createSdkworkCouponService({
                locale: options.locale,
                messages: options.messages,
            }),
            ...options.service,
        }
        : createSdkworkCouponService({
            locale: options.locale,
            messages: options.messages,
        });
    const listeners = new Set();
    let state = normalizeState({
        activeTab: "discover",
        dashboard: fallbackDashboard,
        isBootstrapped: false,
        isDetailLoading: false,
        isDetailOpen: false,
        isLoading: false,
        isMutating: false,
        isRedeemOpen: false,
        selectedCatalogCouponId: fallbackDashboard.catalogCoupons[0]?.id ?? null,
        selectedUserCouponId: fallbackDashboard.availableCoupons[0]?.id ?? fallbackDashboard.myCoupons[0]?.id ?? null,
        visibleCatalogCoupons: fallbackDashboard.catalogCoupons,
        visibleUserCoupons: fallbackDashboard.availableCoupons,
        ...options.initialState,
    }, fallbackDashboard);
    function emit() {
        listeners.forEach((listener) => listener());
    }
    function setState(next) {
        const partial = typeof next === "function" ? next(state) : next;
        state = normalizeState({
            ...state,
            ...partial,
        }, fallbackDashboard);
        emit();
    }
    async function refreshDashboard(options = {}) {
        const dashboard = normalizeDashboard(await service.getDashboard(), state.dashboard);
        setState((currentState) => ({
            activeTab: options.activeTab ?? currentState.activeTab,
            dashboard,
            isBootstrapped: true,
            isLoading: false,
            isMutating: false,
            selectedCatalogCouponId: options.selectedCatalogCouponId === undefined
                ? currentState.selectedCatalogCouponId
                : options.selectedCatalogCouponId,
            selectedUserCouponId: options.selectedUserCouponId === undefined
                ? currentState.selectedUserCouponId
                : options.selectedUserCouponId,
        }));
        return dashboard;
    }
    function resolveOwnedCouponId(userCouponId) {
        const resolvedUserCouponId = userCouponId ?? state.selectedUserCouponId;
        if (!resolvedUserCouponId) {
            throw new Error(copy.selectCouponRequired);
        }
        return stripUserCouponId(resolvedUserCouponId);
    }
    return {
        async bootstrap() {
            setState({
                isLoading: true,
                lastError: undefined,
            });
            try {
                await refreshDashboard();
                return state;
            }
            catch (error) {
                setState({
                    isBootstrapped: true,
                    isLoading: false,
                    lastError: error instanceof Error ? error.message : copy.bootstrapFailed,
                });
                throw error;
            }
        },
        async cancelUseCoupon(userCouponId) {
            const resolvedUserCouponId = resolveOwnedCouponId(userCouponId);
            setState({
                isMutating: true,
                lastError: undefined,
            });
            try {
                const result = await service.cancelUseCoupon(resolvedUserCouponId);
                await refreshDashboard({
                    activeTab: "my",
                    selectedUserCouponId: result.id,
                });
                return result;
            }
            catch (error) {
                setState({
                    isMutating: false,
                    lastError: error instanceof Error ? error.message : copy.cancelUseFailed,
                });
                throw error;
            }
        },
        closeDetail() {
            setState({
                detail: undefined,
                detailKind: undefined,
                isDetailLoading: false,
                isDetailOpen: false,
            });
        },
        closeRedeemDialog() {
            setState({
                isRedeemOpen: false,
            });
        },
        async exchangeCouponByPoints(input) {
            setState({
                isMutating: true,
                lastError: undefined,
            });
            try {
                const result = await service.exchangeCouponByPoints({
                    couponId: stripCouponId(input.couponId),
                });
                await refreshDashboard({
                    activeTab: "my",
                    selectedUserCouponId: result.id,
                });
                return result;
            }
            catch (error) {
                setState({
                    isMutating: false,
                    lastError: error instanceof Error ? error.message : copy.exchangeFailed,
                });
                throw error;
            }
        },
        getState() {
            return state;
        },
        async openCatalogDetail(couponId) {
            setState({
                isDetailLoading: true,
                isDetailOpen: true,
                lastError: undefined,
                selectedCatalogCouponId: couponId,
            });
            try {
                const detail = await service.getCouponDetail(stripCouponId(couponId));
                setState({
                    detail,
                    detailKind: "catalog",
                    isDetailLoading: false,
                    isDetailOpen: true,
                    selectedCatalogCouponId: detail.id,
                });
                return state;
            }
            catch (error) {
                setState({
                    isDetailLoading: false,
                    lastError: error instanceof Error ? error.message : copy.couponDetailFailed,
                });
                throw error;
            }
        },
        openRedeemDialog() {
            setState({
                isRedeemOpen: true,
                lastError: undefined,
            });
        },
        async openUserCouponDetail(userCouponId) {
            setState({
                isDetailLoading: true,
                isDetailOpen: true,
                lastError: undefined,
                selectedUserCouponId: userCouponId.startsWith("user-coupon-") ? userCouponId : `user-coupon-${userCouponId}`,
            });
            try {
                const detail = await service.getUserCouponDetail(stripUserCouponId(userCouponId));
                setState({
                    detail,
                    detailKind: "owned",
                    isDetailLoading: false,
                    isDetailOpen: true,
                    selectedUserCouponId: detail.id,
                });
                return state;
            }
            catch (error) {
                setState({
                    isDetailLoading: false,
                    lastError: error instanceof Error ? error.message : copy.userCouponDetailFailed,
                });
                throw error;
            }
        },
        async receiveCoupon(couponId) {
            setState({
                isMutating: true,
                lastError: undefined,
            });
            try {
                const result = await service.receiveCoupon(stripCouponId(couponId));
                await refreshDashboard({
                    activeTab: "my",
                    selectedUserCouponId: result.id,
                });
                return result;
            }
            catch (error) {
                setState({
                    isMutating: false,
                    lastError: error instanceof Error ? error.message : copy.receiveFailed,
                });
                throw error;
            }
        },
        async redeemCoupon(input) {
            setState({
                isMutating: true,
                lastError: undefined,
            });
            try {
                const result = await service.redeemCoupon(input);
                await refreshDashboard({
                    activeTab: "my",
                    selectedUserCouponId: result.id,
                });
                setState({
                    isRedeemOpen: false,
                });
                return result;
            }
            catch (error) {
                setState({
                    isMutating: false,
                    lastError: error instanceof Error ? error.message : copy.redeemFailed,
                });
                throw error;
            }
        },
        async refresh() {
            await refreshDashboard();
            return state;
        },
        async rollbackPointsExchange(input = {}) {
            const resolvedUserCouponId = resolveOwnedCouponId(input.userCouponId);
            setState({
                isMutating: true,
                lastError: undefined,
            });
            try {
                const result = await service.rollbackPointsExchange({
                    reason: input.reason,
                    userCouponId: resolvedUserCouponId,
                });
                await refreshDashboard({
                    activeTab: "my",
                    selectedUserCouponId: result.id,
                });
                return result;
            }
            catch (error) {
                setState({
                    isMutating: false,
                    lastError: error instanceof Error ? error.message : copy.rollbackFailed,
                });
                throw error;
            }
        },
        selectCatalogCoupon(couponId) {
            setState({
                selectedCatalogCouponId: couponId,
            });
        },
        selectUserCoupon(userCouponId) {
            setState({
                selectedUserCouponId: userCouponId,
            });
        },
        service,
        setTab(tab) {
            setState({
                activeTab: tab,
            });
        },
        subscribe(listener) {
            listeners.add(listener);
            return () => {
                listeners.delete(listener);
            };
        },
        async useCoupon(input) {
            setState({
                isMutating: true,
                lastError: undefined,
            });
            try {
                const result = await service.useCoupon({
                    orderId: input.orderId,
                    userCouponId: resolveOwnedCouponId(input.userCouponId),
                });
                await refreshDashboard({
                    activeTab: "history",
                    selectedUserCouponId: result.id,
                });
                return result;
            }
            catch (error) {
                setState({
                    isMutating: false,
                    lastError: error instanceof Error ? error.message : copy.useFailed,
                });
                throw error;
            }
        },
    };
}
export function useSdkworkCouponController(controller, options) {
    return useMemo(() => controller ?? createSdkworkCouponController({
        ...(options?.locale ? { locale: options.locale } : {}),
        ...(options?.messages ? { messages: options.messages } : {}),
    }), [controller, options?.locale, options?.messages]);
}
export function useSdkworkCouponControllerState(controller) {
    return useSyncExternalStore(controller.subscribe, controller.getState, controller.getState);
}
//# sourceMappingURL=coupon-controller.js.map