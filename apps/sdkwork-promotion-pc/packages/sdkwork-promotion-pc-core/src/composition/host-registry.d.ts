import type { SdkworkPromotionPcHostAdapter } from "../host";
export interface SdkworkPromotionPcHostRegistry {
    readonly adapters: ReadonlyMap<string, SdkworkPromotionPcHostAdapter>;
    register(adapter: SdkworkPromotionPcHostAdapter): void;
    get(name: string): SdkworkPromotionPcHostAdapter | undefined;
    list(): readonly SdkworkPromotionPcHostAdapter[];
}
export declare function createPromotionPcHostRegistry(initial?: readonly SdkworkPromotionPcHostAdapter[]): SdkworkPromotionPcHostRegistry;
//# sourceMappingURL=host-registry.d.ts.map