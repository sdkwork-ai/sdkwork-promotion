export type SdkworkPromotionPcRouteSurface = "app" | "backend-admin";
export interface SdkworkPromotionPcRouteContribution {
    readonly auth: "public" | "required";
    readonly capability: string;
    readonly domain: "promotion";
    readonly id: string;
    readonly packageName: string;
    readonly path: string;
    readonly permissionHint?: string;
    readonly screen: string;
    readonly surface: SdkworkPromotionPcRouteSurface;
    readonly title: string;
    readonly titleKey: string;
}
export declare const sdkworkPromotionPcRuntimeIdentity: {
    readonly appKey: "sdkwork-promotion-pc";
    readonly architecture: "pc-react";
    readonly domain: "commerce";
    readonly capability: "promotion";
    readonly runtimeFamily: "web";
    readonly version: "0.1.0";
};
export declare function createSdkworkPromotionPcRouteRegistry(...routeGroups: readonly (readonly SdkworkPromotionPcRouteContribution[])[]): readonly SdkworkPromotionPcRouteContribution[];
export * from "./host/index.js";
export * from "./modules/index.js";
export * from "./sdk/index.js";
export * from "./session/index.js";
export * from "./composition/index.js";
//# sourceMappingURL=index.d.ts.map