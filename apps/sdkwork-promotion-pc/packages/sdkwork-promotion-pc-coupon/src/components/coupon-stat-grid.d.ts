import type { SdkworkCouponCatalogDigest, SdkworkUserCouponDigest } from "../coupon";
import type { SdkworkCouponStatistics } from "../coupon-service";
export interface SdkworkCouponStatGridProps {
    catalogDigest: SdkworkCouponCatalogDigest;
    statistics: SdkworkCouponStatistics;
    userDigest: SdkworkUserCouponDigest;
}
export declare function SdkworkCouponStatGrid({ catalogDigest, statistics, userDigest, }: SdkworkCouponStatGridProps): import("react").JSX.Element;
//# sourceMappingURL=coupon-stat-grid.d.ts.map