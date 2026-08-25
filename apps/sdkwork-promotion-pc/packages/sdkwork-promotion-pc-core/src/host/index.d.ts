export type SdkworkPromotionPcHostSurface = "browser" | "desktop" | "tablet-ipados" | "tablet-android";
export interface SdkworkPromotionPcHostCapability {
    readonly deepLinks: boolean;
    readonly clipboard: boolean;
    readonly fileDialogs: boolean;
    readonly updater: boolean;
    readonly tray: boolean;
    readonly notifications: boolean;
}
export interface SdkworkPromotionPcHostAdapter {
    readonly name: string;
    readonly surface: SdkworkPromotionPcHostSurface;
    readonly capabilities: SdkworkPromotionPcHostCapability;
    invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
}
export interface SdkworkPromotionPcHostContext {
    readonly adapter: SdkworkPromotionPcHostAdapter | null;
}
export declare function createPromotionPcHostContext(adapter?: SdkworkPromotionPcHostAdapter | null): SdkworkPromotionPcHostContext;
export declare function createPromotionPcBrowserHostAdapter(): SdkworkPromotionPcHostAdapter | null;
//# sourceMappingURL=index.d.ts.map