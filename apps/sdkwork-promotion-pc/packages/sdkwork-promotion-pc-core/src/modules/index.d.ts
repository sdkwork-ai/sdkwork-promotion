export interface SdkworkPromotionPcModuleManifest {
    readonly id: string;
    readonly packageName: string;
    readonly capability: string;
    readonly domain: "commerce";
    readonly version: string;
    readonly surface: "app" | "backend-admin";
    readonly routeIds?: readonly string[];
}
export interface SdkworkPromotionPcModuleRegistry {
    readonly modules: ReadonlyMap<string, SdkworkPromotionPcModuleManifest>;
    register(manifest: SdkworkPromotionPcModuleManifest): void;
    get(id: string): SdkworkPromotionPcModuleManifest | undefined;
    list(): readonly SdkworkPromotionPcModuleManifest[];
}
export declare function createPromotionPcModuleManifest(manifest: SdkworkPromotionPcModuleManifest): SdkworkPromotionPcModuleManifest;
//# sourceMappingURL=index.d.ts.map