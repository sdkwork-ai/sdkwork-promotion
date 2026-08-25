export declare const APP_PROMOTION_METHOD_TREE: {
    readonly promotions: {
        readonly userCoupons: {
            readonly list: true;
            readonly retrieve: true;
            readonly claims: {
                readonly create: true;
            };
            readonly wallet: {
                readonly list: true;
                readonly retrieve: true;
            };
        };
        readonly offers: {
            readonly list: true;
            readonly retrieve: true;
        };
        readonly codes: {
            readonly redemptions: {
                readonly create: true;
            };
        };
        readonly discountApplications: {
            readonly create: true;
            readonly settle: true;
            readonly release: true;
            readonly rollback: true;
            readonly reversals: {
                readonly create: true;
            };
        };
    };
};
export type PromotionRequestParams = Record<string, unknown>;
export type PromotionSdkResponse<T> = Promise<T | {
    code?: number | string;
    data?: T;
    message?: string;
    msg?: string;
}>;
export type PromotionSdkMethod = (...args: any[]) => PromotionSdkResponse<any>;
type MethodTree = {
    readonly [key: string]: true | MethodTree;
};
export type ClientFromMethodTree<TTree extends MethodTree> = {
    readonly [TKey in keyof TTree]: TTree[TKey] extends true ? PromotionSdkMethod : TTree[TKey] extends MethodTree ? ClientFromMethodTree<TTree[TKey]> : never;
};
export type PromotionAppSdkClient = {
    commerce: ClientFromMethodTree<typeof APP_PROMOTION_METHOD_TREE>;
};
export {};
//# sourceMappingURL=index.d.ts.map