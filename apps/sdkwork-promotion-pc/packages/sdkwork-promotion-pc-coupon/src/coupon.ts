import {
  toNullableSdkworkPromotionNumber,
  toSdkworkPromotionNumber,
  toSdkworkPromotionOptionalString,
} from "@sdkwork/promotion-service";

export interface SdkworkAppCapabilityManifest {
  description: string;
  host?: string;
  id: string;
  packageNames: string[];
  theme?: string;
  title: string;
}

export interface CreateSdkworkAppCapabilityManifestOptions {
  description?: string;
  host?: string;
  id?: string;
  packageNames?: string[];
  theme?: string;
  title?: string;
}

function createSdkworkAppCapabilityManifest(
  options: CreateSdkworkAppCapabilityManifestOptions = {},
): SdkworkAppCapabilityManifest {
  return {
    description: options.description ?? "",
    ...(options.host ? { host: options.host } : {}),
    id: options.id ?? "sdkwork-capability",
    packageNames: [...(options.packageNames ?? [])],
    ...(options.theme ? { theme: options.theme } : {}),
    title: options.title ?? "Capability",
  };
}

export type SdkworkCouponTab = "discover" | "history" | "my";
export type SdkworkCouponStatus = "available" | "expired" | "inactive" | "used";
export type SdkworkCouponType = "cash" | "discount" | "gift" | "points-exchange" | "unknown";
export type SdkworkCouponAcquireType = "admin-grant" | "points-exchange" | "receive" | "redeem-code" | "unknown";

export interface SdkworkCouponWorkspaceManifest extends SdkworkAppCapabilityManifest {
  capability: "coupon";
  routePath: string;
}

export interface CreateCouponWorkspaceManifestOptions
  extends Partial<
    Pick<CreateSdkworkAppCapabilityManifestOptions, "description" | "host" | "id" | "packageNames" | "theme" | "title">
  > {
  routePath?: string;
}

export interface SdkworkCouponRouteIntent {
  couponId?: string;
  focusWindow: boolean;
  route: string;
  source: "coupon-workspace";
  tab?: SdkworkCouponTab;
  type: "coupon-route-intent";
  userCouponId?: string;
}

export interface CreateCouponRouteIntentOptions {
  basePath?: string;
  couponId?: string;
  focusWindow?: boolean;
  tab?: SdkworkCouponTab;
  userCouponId?: string;
}

export interface SdkworkCouponCatalog {
  amountCny: number | null;
  benefitAmount?: string;
  benefitCurrencyCode?: string;
  benefitKind?: string;
  canReceive: boolean;
  couponId?: string;
  description?: string;
  discountRate: number | null;
  endTime?: string;
  getLimit: number | null;
  id: string;
  minimumSpendCny: number | null;
  name: string;
  pointCost: number | null;
  pointsExchange: boolean;
  receivedCount: number | null;
  remainingCount: number | null;
  scopeType?: string;
  scopeValue?: string;
  stackable: boolean;
  startTime?: string;
  status: SdkworkCouponStatus;
  statusLabel?: string;
  total: number | null;
  type: SdkworkCouponType;
  typeLabel?: string;
  usedCount: number | null;
}

export interface SdkworkUserCoupon {
  acquireAt?: string;
  acquireType?: SdkworkCouponAcquireType;
  amountCny: number | null;
  available: boolean;
  benefitAmount?: string;
  benefitCurrencyCode?: string;
  benefitKind?: string;
  code?: string;
  couponId?: string;
  expireAt?: string;
  id: string;
  minimumSpendCny: number | null;
  name: string;
  orderId?: string;
  pointCost: number | null;
  pointsRefundAt?: string;
  pointsRefunded: boolean;
  remainingDays: number | null;
  scopeType?: string;
  scopeValue?: string;
  status: SdkworkCouponStatus;
  statusLabel?: string;
  type: SdkworkCouponType;
  typeLabel?: string;
  useAt?: string;
  userCouponId?: string;
  discountRate?: number | null;
}

export interface SdkworkCouponCatalogDigestInput {
  canReceive: boolean;
  id: string;
  pointsExchange: boolean;
  status: SdkworkCouponStatus;
}

export interface SdkworkCouponCatalogDigest {
  claimableCoupons: number;
  pointsExchangeCoupons: number;
  totalCoupons: number;
}

export interface SdkworkUserCouponDigestInput {
  discountAmountCny?: number | null;
  id: string;
  remainingDays?: number | null;
  status: SdkworkCouponStatus;
}

export interface SdkworkUserCouponDigest {
  availableCoupons: number;
  expiringSoonCoupons: number;
  highestDiscountAmountCny: number;
  totalCoupons: number;
}

export interface SdkworkCouponDiscountInput {
  discountAmountCny?: number | null;
  discountRate?: number | null;
  id: string;
  minimumSpendCny?: number | null;
  name: string;
  status?: SdkworkCouponStatus;
}

export interface SdkworkRemoteCouponLike {
  amount?: unknown;
  benefitAmount?: unknown;
  benefitCurrencyCode?: unknown;
  benefitKind?: unknown;
  canReceive?: unknown;
  couponId?: unknown;
  description?: unknown;
  discount?: unknown;
  getLimit?: unknown;
  minConsume?: unknown;
  name?: unknown;
  pointCost?: unknown;
  pointsExchange?: unknown;
  receivedCount?: unknown;
  remainingCount?: unknown;
  scopeType?: unknown;
  scopeValue?: unknown;
  stackable?: unknown;
  startTime?: unknown;
  endTime?: unknown;
  status?: unknown;
  statusName?: unknown;
  total?: unknown;
  type?: unknown;
  typeName?: unknown;
  usedCount?: unknown;
}

export interface SdkworkRemoteUserCouponLike {
  acquireAt?: unknown;
  acquireType?: unknown;
  amount?: unknown;
  available?: unknown;
  benefitAmount?: unknown;
  benefitCurrencyCode?: unknown;
  benefitKind?: unknown;
  code?: unknown;
  couponCode?: unknown;
  couponId?: unknown;
  couponName?: unknown;
  discount?: unknown;
  expireAt?: unknown;
  id?: unknown;
  minConsume?: unknown;
  name?: unknown;
  orderId?: unknown;
  pointCost?: unknown;
  pointsRefundAt?: unknown;
  pointsRefunded?: unknown;
  remainingDays?: unknown;
  scopeType?: unknown;
  scopeValue?: unknown;
  status?: unknown;
  statusName?: unknown;
  type?: unknown;
  typeName?: unknown;
  useAt?: unknown;
  userCouponId?: unknown;
}

function normalizeBasePath(basePath: string | undefined): string {
  const normalized = (basePath ?? "/coupons").trim();
  if (!normalized || normalized === "/") {
    return "/coupons";
  }

  return normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
}

function normalizeDiscountMultiplier(rate: number | null | undefined): number | null {
  if (typeof rate !== "number" || !Number.isFinite(rate) || rate <= 0) {
    return null;
  }

  if (rate <= 1) {
    return rate;
  }

  if (rate <= 10) {
    return rate / 10;
  }

  if (rate <= 100) {
    return rate / 100;
  }

  return null;
}

export function normalizeSdkworkCouponType(value: unknown): SdkworkCouponType {
  const normalized = (toSdkworkPromotionOptionalString(value) || "").toUpperCase();

  if (normalized === "CASH") {
    return "cash";
  }

  if (normalized === "DISCOUNT") {
    return "discount";
  }

  if (normalized === "GIFT") {
    return "gift";
  }

  if (normalized === "POINTS_EXCHANGE") {
    return "points-exchange";
  }

  return "unknown";
}

export function normalizeSdkworkCouponAcquireType(value: unknown): SdkworkCouponAcquireType {
  const normalized = (toSdkworkPromotionOptionalString(value) || "").toUpperCase();

  if (normalized === "RECEIVE") {
    return "receive";
  }

  if (normalized === "REDEEM_CODE") {
    return "redeem-code";
  }

  if (normalized === "POINTS_EXCHANGE") {
    return "points-exchange";
  }

  if (normalized === "ADMIN_GRANT") {
    return "admin-grant";
  }

  return "unknown";
}

export function normalizeSdkworkCouponStatus(value: unknown, available?: unknown): SdkworkCouponStatus {
  const normalized = (toSdkworkPromotionOptionalString(value) || "").toUpperCase();

  if (
    available === true
    || normalized === "UNUSED"
    || normalized === "AVAILABLE"
    || normalized === "ACTIVE"
    // A claimed wallet coupon is still usable; the promotion API returns the
    // persisted `claimed`/`pending`/`issued` status, not an `available` flag.
    || normalized === "CLAIMED"
    || normalized === "PENDING"
    || normalized === "ISSUED"
  ) {
    return "available";
  }

  if (normalized === "USED" || normalized === "REDEEMED") {
    return "used";
  }

  if (normalized === "EXPIRED") {
    return "expired";
  }

  return "inactive";
}

export function normalizeSdkworkRemoteCoupon(
  coupon: SdkworkRemoteCouponLike,
  index = 0,
): SdkworkCouponCatalog {
  const couponId = toSdkworkPromotionOptionalString(coupon.couponId);

  return {
    amountCny: toNullableSdkworkPromotionNumber(coupon.amount),
    benefitAmount: toSdkworkPromotionOptionalString(coupon.benefitAmount),
    benefitCurrencyCode: toSdkworkPromotionOptionalString(coupon.benefitCurrencyCode),
    benefitKind: toSdkworkPromotionOptionalString(coupon.benefitKind),
    canReceive: coupon.canReceive !== false && normalizeSdkworkCouponStatus(coupon.status, coupon.canReceive) === "available",
    couponId,
    description: toSdkworkPromotionOptionalString(coupon.description),
    discountRate: toNullableSdkworkPromotionNumber(coupon.discount),
    endTime: toSdkworkPromotionOptionalString(coupon.endTime),
    getLimit: toNullableSdkworkPromotionNumber(coupon.getLimit),
    id: `coupon-${couponId || index + 1}`,
    minimumSpendCny: toNullableSdkworkPromotionNumber(coupon.minConsume),
    name: toSdkworkPromotionOptionalString(coupon.name) || "Coupon",
    pointCost: toNullableSdkworkPromotionNumber(coupon.pointCost),
    pointsExchange: coupon.pointsExchange === true,
    receivedCount: toNullableSdkworkPromotionNumber(coupon.receivedCount),
    remainingCount: toNullableSdkworkPromotionNumber(coupon.remainingCount),
    scopeType: toSdkworkPromotionOptionalString(coupon.scopeType),
    scopeValue: toSdkworkPromotionOptionalString(coupon.scopeValue),
    stackable: coupon.stackable === true,
    startTime: toSdkworkPromotionOptionalString(coupon.startTime),
    status: normalizeSdkworkCouponStatus(coupon.status, coupon.canReceive),
    statusLabel: toSdkworkPromotionOptionalString(coupon.statusName),
    total: toNullableSdkworkPromotionNumber(coupon.total),
    type: normalizeSdkworkCouponType(coupon.type),
    typeLabel: toSdkworkPromotionOptionalString(coupon.typeName),
    usedCount: toNullableSdkworkPromotionNumber(coupon.usedCount),
  };
}

export function normalizeSdkworkRemoteUserCoupon(
  coupon: SdkworkRemoteUserCouponLike,
  index = 0,
): SdkworkUserCoupon {
  const rawId = toSdkworkPromotionOptionalString(coupon.id);
  const couponId = toSdkworkPromotionOptionalString(coupon.couponId) ?? rawId;
  const userCouponId = toSdkworkPromotionOptionalString(coupon.userCouponId) ?? rawId;

  return {
    acquireAt: toSdkworkPromotionOptionalString(coupon.acquireAt),
    acquireType: normalizeSdkworkCouponAcquireType(coupon.acquireType),
    amountCny: toNullableSdkworkPromotionNumber(coupon.amount),
    available: coupon.available === true || normalizeSdkworkCouponStatus(coupon.status, coupon.available) === "available",
    benefitAmount: toSdkworkPromotionOptionalString(coupon.benefitAmount),
    benefitCurrencyCode: toSdkworkPromotionOptionalString(coupon.benefitCurrencyCode),
    benefitKind: toSdkworkPromotionOptionalString(coupon.benefitKind),
    // The promotion app API returns `code`; the admin/composed shape returns
    // `couponCode`. Accept both so a claimed wallet coupon shows its redeemable
    // code instead of a blank value.
    code: toSdkworkPromotionOptionalString(coupon.couponCode)
      ?? toSdkworkPromotionOptionalString(coupon.code),
    couponId,
    discountRate: toNullableSdkworkPromotionNumber(coupon.discount),
    expireAt: toSdkworkPromotionOptionalString(coupon.expireAt),
    id: `user-coupon-${userCouponId || couponId || index + 1}`,
    minimumSpendCny: toNullableSdkworkPromotionNumber(coupon.minConsume),
    name: toSdkworkPromotionOptionalString(coupon.couponName)
      ?? toSdkworkPromotionOptionalString(coupon.name)
      ?? "Coupon",
    orderId: toSdkworkPromotionOptionalString(coupon.orderId),
    pointCost: toNullableSdkworkPromotionNumber(coupon.pointCost),
    pointsRefundAt: toSdkworkPromotionOptionalString(coupon.pointsRefundAt),
    pointsRefunded: coupon.pointsRefunded === true,
    remainingDays: toNullableSdkworkPromotionNumber(coupon.remainingDays),
    scopeType: toSdkworkPromotionOptionalString(coupon.scopeType),
    scopeValue: toSdkworkPromotionOptionalString(coupon.scopeValue),
    status: normalizeSdkworkCouponStatus(coupon.status, coupon.available),
    statusLabel: toSdkworkPromotionOptionalString(coupon.statusName),
    type: normalizeSdkworkCouponType(coupon.type),
    typeLabel: toSdkworkPromotionOptionalString(coupon.typeName),
    useAt: toSdkworkPromotionOptionalString(coupon.useAt),
    userCouponId,
  };
}

export interface SdkworkCouponBenefitLabels {
  cash: string;
  points: string;
  subscription: string;
  tokenBank: string;
}

export interface SdkworkCouponBenefitShape {
  benefitAmount?: string;
  benefitCurrencyCode?: string;
  benefitKind?: string;
}

/// Formats the frozen asset benefit shared by catalogue and wallet coupons.
/// Returns `null` for a discount coupon so the caller keeps the money value.
export function formatSdkworkCouponBenefit(
  coupon: SdkworkCouponBenefitShape,
  labels: SdkworkCouponBenefitLabels,
): string | null {
  if (!coupon.benefitKind || !coupon.benefitAmount) {
    return null;
  }
  switch (coupon.benefitKind) {
    case "token_bank_credit":
      return `${coupon.benefitAmount} ${labels.tokenBank}`;
    case "points_credit":
      return `${coupon.benefitAmount} ${labels.points}`;
    case "cash_credit": {
      const minor = Number(coupon.benefitAmount);
      if (!Number.isFinite(minor)) {
        return null;
      }
      return `${coupon.benefitCurrencyCode ?? labels.cash} ${(minor / 100).toFixed(2)}`.trim();
    }
    case "subscription":
      return `${coupon.benefitAmount} ${labels.subscription}`;
    default:
      return null;
  }
}

export function formatSdkworkUserCouponBenefit(
  coupon: SdkworkUserCoupon,
  labels: SdkworkCouponBenefitLabels,
): string | null {
  return formatSdkworkCouponBenefit(coupon, labels);
}

export function sortSdkworkCouponCatalog(coupons: readonly SdkworkCouponCatalog[]): SdkworkCouponCatalog[] {
  return [...coupons].sort(
    (left, right) =>
      Number(right.canReceive) - Number(left.canReceive)
      || Number(right.pointsExchange) - Number(left.pointsExchange)
      || toSdkworkPromotionNumber(right.amountCny) - toSdkworkPromotionNumber(left.amountCny)
      || toSdkworkPromotionNumber(right.discountRate) - toSdkworkPromotionNumber(left.discountRate)
      || toSdkworkPromotionNumber(left.pointCost ?? Number.MAX_SAFE_INTEGER) - toSdkworkPromotionNumber(right.pointCost ?? Number.MAX_SAFE_INTEGER)
      || left.name.localeCompare(right.name),
  );
}

export function sortSdkworkUserCoupons(coupons: readonly SdkworkUserCoupon[]): SdkworkUserCoupon[] {
  return [...coupons].sort(
    (left, right) =>
      Number(right.status === "available") - Number(left.status === "available")
      || toSdkworkPromotionNumber(right.amountCny) - toSdkworkPromotionNumber(left.amountCny)
      || toSdkworkPromotionNumber(right.discountRate) - toSdkworkPromotionNumber(left.discountRate)
      || toSdkworkPromotionNumber(left.remainingDays ?? Number.MAX_SAFE_INTEGER) - toSdkworkPromotionNumber(right.remainingDays ?? Number.MAX_SAFE_INTEGER)
      || left.name.localeCompare(right.name),
  );
}

export function estimateSdkworkCouponDiscountAmount(
  amountCny: number,
  coupon: SdkworkCouponDiscountInput | null | undefined,
): number {
  if (!coupon || coupon.status === "expired" || coupon.status === "inactive" || coupon.status === "used") {
    return 0;
  }

  const priceCny = Math.max(toSdkworkPromotionNumber(amountCny), 0);
  if (priceCny <= 0) {
    return 0;
  }

  const minimumSpendCny = coupon.minimumSpendCny ?? null;
  if (minimumSpendCny !== null && priceCny < minimumSpendCny) {
    return 0;
  }

  const fixedDiscount = Math.max(toSdkworkPromotionNumber(coupon.discountAmountCny), 0);
  if (fixedDiscount > 0) {
    return Math.min(fixedDiscount, priceCny);
  }

  const discountMultiplier = normalizeDiscountMultiplier(coupon.discountRate ?? null);
  if (discountMultiplier === null) {
    return 0;
  }

  return Math.max(0, Math.min(priceCny, Math.round((priceCny * (1 - discountMultiplier)) * 100) / 100));
}

export function summarizeSdkworkCouponCatalog(
  coupons: readonly SdkworkCouponCatalogDigestInput[],
): SdkworkCouponCatalogDigest {
  return coupons.reduce<SdkworkCouponCatalogDigest>(
    (summary, coupon) => {
      summary.totalCoupons += 1;

      if (coupon.canReceive && coupon.status === "available") {
        summary.claimableCoupons += 1;
      }

      if (coupon.pointsExchange) {
        summary.pointsExchangeCoupons += 1;
      }

      return summary;
    },
    {
      claimableCoupons: 0,
      pointsExchangeCoupons: 0,
      totalCoupons: 0,
    },
  );
}

export function summarizeSdkworkUserCoupons(
  coupons: readonly SdkworkUserCouponDigestInput[],
): SdkworkUserCouponDigest {
  return coupons.reduce<SdkworkUserCouponDigest>(
    (summary, coupon) => {
      summary.totalCoupons += 1;
      summary.highestDiscountAmountCny = Math.max(summary.highestDiscountAmountCny, toSdkworkPromotionNumber(coupon.discountAmountCny));

      if (coupon.status === "available") {
        summary.availableCoupons += 1;
        if ((coupon.remainingDays ?? null) !== null && (coupon.remainingDays ?? Number.MAX_SAFE_INTEGER) <= 7) {
          summary.expiringSoonCoupons += 1;
        }
      }

      return summary;
    },
    {
      availableCoupons: 0,
      expiringSoonCoupons: 0,
      highestDiscountAmountCny: 0,
      totalCoupons: 0,
    },
  );
}

export function resolveSdkworkUserCouponRequestId(
  coupon: Pick<SdkworkUserCoupon, "couponId" | "id" | "userCouponId">,
): string | undefined {
  if (coupon.userCouponId) {
    return coupon.userCouponId;
  }

  if (coupon.couponId) {
    return coupon.couponId;
  }

  if (coupon.id.startsWith("user-coupon-")) {
    return coupon.id.slice("user-coupon-".length);
  }

  return undefined;
}

export function createCouponWorkspaceManifest({
  description = "Coupon workspace for discovery, redemption, points exchange, and reusable checkout discount operations.",
  host,
  id = "sdkwork-coupon",
  packageNames = ["@sdkwork/promotion-pc-coupon"],
  routePath = "/coupons",
  theme,
  title = "Coupons",
}: CreateCouponWorkspaceManifestOptions = {}): SdkworkCouponWorkspaceManifest {
  return {
    ...createSdkworkAppCapabilityManifest({
      description,
      host,
      id,
      packageNames,
      theme,
      title,
    }),
    capability: "coupon",
    routePath: normalizeBasePath(routePath),
  };
}

export function createCouponRouteIntent(
  options: CreateCouponRouteIntentOptions = {},
): SdkworkCouponRouteIntent {
  const basePath = normalizeBasePath(options.basePath);
  const queryParams = new URLSearchParams();

  if (options.tab) {
    queryParams.set("tab", options.tab);
  }

  if (options.couponId) {
    queryParams.set("couponId", options.couponId);
  }

  if (options.userCouponId) {
    queryParams.set("userCouponId", options.userCouponId);
  }

  const querySuffix = queryParams.toString() ? `?${queryParams.toString()}` : "";

  return {
    ...(options.couponId ? { couponId: options.couponId } : {}),
    focusWindow: options.focusWindow !== false,
    route: `${basePath}${querySuffix}`,
    source: "coupon-workspace",
    ...(options.tab ? { tab: options.tab } : {}),
    type: "coupon-route-intent",
    ...(options.userCouponId ? { userCouponId: options.userCouponId } : {}),
  };
}

export const couponPackageMeta = {
  architecture: "pc-react",
  domain: "commerce",
  package: "@sdkwork/promotion-pc-coupon",
  status: "ready",
} as const;

export type CouponPackageMeta = typeof couponPackageMeta;
