import { type SdkworkCouponDashboardData, type SdkworkCouponPointsExchangeInput, type SdkworkCouponRedeemInput, type SdkworkCouponRollbackInput, type SdkworkCouponService, type SdkworkCouponUseInput } from "./coupon-service";
import type { SdkworkCouponCatalog, SdkworkCouponTab, SdkworkUserCoupon } from "./coupon";
import { type SdkworkCouponMessagesOverrides } from "./coupon-copy";
export interface SdkworkCouponControllerState {
    activeTab: SdkworkCouponTab;
    dashboard: SdkworkCouponDashboardData;
    detail?: SdkworkCouponCatalog | SdkworkUserCoupon;
    detailKind?: "catalog" | "owned";
    isBootstrapped: boolean;
    isDetailLoading: boolean;
    isDetailOpen: boolean;
    isLoading: boolean;
    isMutating: boolean;
    isRedeemOpen: boolean;
    lastError?: string;
    selectedCatalogCouponId: string | null;
    selectedUserCouponId: string | null;
    visibleCatalogCoupons: SdkworkCouponCatalog[];
    visibleUserCoupons: SdkworkUserCoupon[];
}
export interface SdkworkCouponController {
    bootstrap(): Promise<SdkworkCouponControllerState>;
    cancelUseCoupon(userCouponId?: string): Promise<SdkworkUserCoupon>;
    closeDetail(): void;
    closeRedeemDialog(): void;
    exchangeCouponByPoints(input: Pick<SdkworkCouponPointsExchangeInput, "couponId">): Promise<SdkworkUserCoupon>;
    getState(): SdkworkCouponControllerState;
    openCatalogDetail(couponId: string): Promise<SdkworkCouponControllerState>;
    openRedeemDialog(): void;
    openUserCouponDetail(userCouponId: string): Promise<SdkworkCouponControllerState>;
    receiveCoupon(couponId: string): Promise<SdkworkUserCoupon>;
    redeemCoupon(input: SdkworkCouponRedeemInput): Promise<SdkworkUserCoupon>;
    refresh(): Promise<SdkworkCouponControllerState>;
    rollbackPointsExchange(input?: Partial<SdkworkCouponRollbackInput>): Promise<SdkworkUserCoupon>;
    selectCatalogCoupon(couponId: string | null): void;
    selectUserCoupon(userCouponId: string | null): void;
    service: SdkworkCouponService;
    setTab(tab: SdkworkCouponTab): void;
    subscribe(listener: () => void): () => void;
    useCoupon(input: SdkworkCouponUseInput): Promise<SdkworkUserCoupon>;
}
export interface CreateSdkworkCouponControllerOptions {
    initialState?: Partial<SdkworkCouponControllerState>;
    locale?: string | null;
    messages?: SdkworkCouponMessagesOverrides;
    service?: Partial<SdkworkCouponService>;
}
export declare function createSdkworkCouponController(options?: CreateSdkworkCouponControllerOptions): SdkworkCouponController;
export declare function useSdkworkCouponController(controller?: SdkworkCouponController, options?: Pick<CreateSdkworkCouponControllerOptions, "locale" | "messages">): SdkworkCouponController;
export declare function useSdkworkCouponControllerState(controller: SdkworkCouponController): SdkworkCouponControllerState;
//# sourceMappingURL=coupon-controller.d.ts.map