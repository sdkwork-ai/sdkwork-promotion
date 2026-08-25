export const APP_PROMOTION_METHOD_TREE = {
    promotions: {
        userCoupons: {
            list: true,
            retrieve: true,
            claims: { create: true },
            wallet: {
                list: true,
                retrieve: true,
            },
        },
        offers: {
            list: true,
            retrieve: true,
        },
        codes: {
            redemptions: { create: true },
        },
        discountApplications: {
            create: true,
            settle: true,
            release: true,
            rollback: true,
            reversals: { create: true },
        },
    },
};
//# sourceMappingURL=index.js.map