export interface SdkworkPromotionPcSessionTokens {
    readonly accessToken?: string;
    readonly authToken?: string;
    readonly refreshToken?: string;
}
export type SdkworkPromotionPcSessionTokenProvider = () => SdkworkPromotionPcSessionTokens;
export interface SdkworkPromotionPcSessionContext {
    readonly tokenProvider: SdkworkPromotionPcSessionTokenProvider;
    readonly isAuthenticated: () => boolean;
}
export declare function createPromotionPcSessionContext(tokenProvider?: SdkworkPromotionPcSessionTokenProvider): SdkworkPromotionPcSessionContext;
//# sourceMappingURL=index.d.ts.map