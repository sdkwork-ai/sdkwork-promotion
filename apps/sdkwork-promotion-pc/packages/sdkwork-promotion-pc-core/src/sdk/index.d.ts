export type SdkworkPromotionPcSdkSurface = "app" | "backend-admin";
export interface SdkworkPromotionPcSdkClientEntry {
    readonly name: string;
    readonly surface: SdkworkPromotionPcSdkSurface;
    readonly packageName: string;
    readonly apiPrefix: string;
    readonly capability: string;
}
export interface SdkworkPromotionPcSdkClientRegistry {
    readonly clients: ReadonlyMap<string, SdkworkPromotionPcSdkClientEntry>;
    register(entry: SdkworkPromotionPcSdkClientEntry): void;
    get(name: string): SdkworkPromotionPcSdkClientEntry | undefined;
    list(): readonly SdkworkPromotionPcSdkClientEntry[];
}
export declare function createPromotionPcSdkClientRegistry(initial?: readonly SdkworkPromotionPcSdkClientEntry[]): SdkworkPromotionPcSdkClientRegistry;
//# sourceMappingURL=index.d.ts.map