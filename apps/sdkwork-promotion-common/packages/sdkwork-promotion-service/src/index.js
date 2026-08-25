import { APP_PROMOTION_METHOD_TREE, } from "@sdkwork/promotion-sdk-ports";
import { formatMoney } from "@sdkwork/utils/money";
function unwrapPromotionPage(value, page, pageSize) {
    const record = value && typeof value === "object" ? value : {};
    const data = record.data && typeof record.data === "object"
        ? record.data
        : record;
    const pageInfo = data.pageInfo && typeof data.pageInfo === "object"
        ? data.pageInfo
        : {};
    return {
        items: Array.isArray(data.items) ? data.items : [],
        page: Number(pageInfo.page ?? page),
        pageSize: Number(pageInfo.pageSize ?? pageSize),
        totalItems: Number(pageInfo.totalItems ?? 0),
        totalPages: Number(pageInfo.totalPages ?? 0),
    };
}
function toPromotionListParams(query = {}) {
    return {
        page: query.page ?? 1,
        pageSize: query.pageSize ?? 20,
        q: query.q?.trim() || undefined,
        status: query.status,
    };
}
export function createSdkworkPromotionBackendService(client) {
    const list = async (loader, query = {}) => {
        const page = query.page ?? 1;
        const pageSize = query.pageSize ?? 20;
        return unwrapPromotionPage(await loader(toPromotionListParams(query)), page, pageSize);
    };
    return {
        getOverview: () => client.promotions.overview.retrieve(),
        listCampaigns: (query) => list((params) => client.promotions.campaigns.list(params), query),
        getCampaign: (campaignId) => client.promotions.campaigns.retrieve(campaignId),
        createCampaign: (input) => client.promotions.campaigns.create(input),
        updateCampaign: (campaignId, input) => client.promotions.campaigns.update(campaignId, input),
        deleteCampaign: (campaignId) => client.promotions.campaigns.delete(campaignId),
        listOffers: (query) => list((params) => client.promotions.offers.list(params), query),
        getOffer: (offerId) => client.promotions.offers.retrieve(offerId),
        createOffer: (input) => client.promotions.offers.create(input),
        updateOffer: (offerId, input) => client.promotions.offers.update(offerId, input),
        async updateOfferStatus(offerId, status) {
            await client.promotions.offers.status.update(offerId, { status });
        },
        deleteOffer: (offerId) => client.promotions.offers.delete(offerId),
        listCouponStocks: (query) => list((params) => client.promotions.couponStocks.list(params), query),
        createCouponStock: (input) => client.promotions.couponStocks.create(input),
        listCodeBatches: (query) => list((params) => client.promotions.codeBatches.list(params), query),
        createCodeBatch: (input) => client.promotions.codeBatches.create(input),
        listCodes: (query) => list((params) => client.promotions.codes.list(params), query),
        listDistributionTasks: (query) => list((params) => client.promotions.distributionTasks.list(params), query),
        createDistributionTask: (input) => client.promotions.distributionTasks.create(input),
        listUserCoupons: (query) => list((params) => client.promotions.userCoupons.list(params), query),
        listCouponLedger: (query) => list((params) => client.promotions.couponLedgerEntries.list(params), query),
        listDiscountApplications: (query) => list((params) => client.promotions.discountApplications.list(params), query),
    };
}
let sdkworkPromotionAppServiceProvider = null;
let sdkworkPromotionSessionTokenProvider = () => ({});
export function configureSdkworkPromotionAppServiceProvider(provider) {
    sdkworkPromotionAppServiceProvider = provider;
}
export function configureSdkworkPromotionSessionTokenProvider(provider) {
    sdkworkPromotionSessionTokenProvider = provider ?? (() => ({}));
}
export function getSdkworkPromotionService() {
    if (!sdkworkPromotionAppServiceProvider) {
        throw new Error("SDKWork promotion service provider is not configured. Call configureSdkworkPromotionAppServiceProvider() from promotion PC bootstrap.");
    }
    return sdkworkPromotionAppServiceProvider();
}
export function getSdkworkPromotionSessionTokens() {
    const tokens = sdkworkPromotionSessionTokenProvider();
    return {
        accessToken: normalizeSessionToken(tokens.accessToken),
        authToken: normalizeSessionToken(tokens.authToken),
        refreshToken: normalizeSessionToken(tokens.refreshToken),
    };
}
export function hasSdkworkPromotionSession() {
    const tokens = getSdkworkPromotionSessionTokens();
    return Boolean(normalizeSessionToken(tokens.authToken) || normalizeSessionToken(tokens.accessToken));
}
export function requireSdkworkPromotionSession(message = "Authentication required") {
    if (!hasSdkworkPromotionSession()) {
        throw new Error(message);
    }
}
export function createSdkworkPromotionAppService(input) {
    return {
        promotions: buildServiceTree(APP_PROMOTION_METHOD_TREE.promotions, input.appClient.commerce.promotions, ["commerce", "promotions"]),
    };
}
export function unwrapSdkworkPromotionResponse(value, fallbackMessage = "Request failed.") {
    if (!value || typeof value !== "object") {
        return value;
    }
    if (!("data" in value) && !("code" in value)) {
        return value;
    }
    const envelope = value;
    if (!isSuccessCode(envelope.code)) {
        throw new Error(String(envelope.message || envelope.msg || fallbackMessage).trim());
    }
    return (envelope.data ?? null);
}
export function toSdkworkPromotionOptionalString(value) {
    const normalized = typeof value === "string" ? value.trim() : String(value ?? "").trim();
    return normalized || undefined;
}
export function toNullableSdkworkPromotionNumber(value) {
    if (typeof value === "number" && Number.isFinite(value)) {
        return value;
    }
    if (typeof value === "string" && value.trim()) {
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : null;
    }
    return null;
}
export function toSdkworkPromotionNumber(value, fallback = 0) {
    return toNullableSdkworkPromotionNumber(value) ?? fallback;
}
export function toSdkworkPromotionMutationStatus(status) {
    const normalized = String(status ?? "").trim().toUpperCase();
    if (normalized === "SUCCESS" || normalized === "COMPLETED" || normalized === "PAID") {
        return "completed";
    }
    if (normalized === "FAILED" || normalized === "REJECTED") {
        return "failed";
    }
    return "pending";
}
export function formatSdkworkPromotionCurrencyCny(value, language = "en-US") {
    return formatMoney(value, { currency: "CNY", locale: language, mode: "symbol" }) ?? "--";
}
export function formatSdkworkPromotionPoints(value, language = "en-US") {
    return new Intl.NumberFormat(language).format(value);
}
export function formatSdkworkPromotionPointsRate(points, language = "en-US") {
    return language === "zh-CN"
        ? `${formatSdkworkPromotionPoints(points, language)} \u79ef\u5206 / 1 \u5143`
        : `${formatSdkworkPromotionPoints(points, language)} pts / CNY 1`;
}
export function formatSdkworkPromotionPointsDelta(value, language = "en-US") {
    const formatted = formatSdkworkPromotionPoints(Math.abs(value), language);
    if (value > 0) {
        return `+${formatted}`;
    }
    if (value < 0) {
        return `-${formatted}`;
    }
    return "0";
}
function buildServiceTree(template, client, missingPathPrefix, servicePath = []) {
    const service = {};
    for (const [key, marker] of Object.entries(template)) {
        const nextServicePath = [...servicePath, key];
        if (marker === true) {
            const missingPath = [...missingPathPrefix, ...nextServicePath].join(".");
            service[key] = (...args) => callPromotion(readMethod(client, nextServicePath), missingPath, ...args);
        }
        else {
            service[key] = buildServiceTree(marker, client, missingPathPrefix, nextServicePath);
        }
    }
    return service;
}
function readMethod(root, path) {
    let node = root;
    for (const segment of path) {
        if (!node || typeof node !== "object") {
            return undefined;
        }
        const parent = node;
        node = parent[segment];
        if (typeof node === "function") {
            return node.bind(parent);
        }
    }
    return typeof node === "function" ? node : undefined;
}
async function callPromotion(method, name, ...args) {
    if (!method) {
        throw new Error(`Missing SDKWork promotion SDK resource: ${name}`);
    }
    return method(...args);
}
function normalizeSessionToken(value) {
    const normalized = typeof value === "string" ? value.trim() : "";
    return normalized || undefined;
}
function isSuccessCode(code) {
    if (code === undefined || code === null || code === "") {
        return true;
    }
    if (typeof code === "number") {
        return code === 0 || code === 200 || code === 2000;
    }
    const normalized = String(code).trim();
    return normalized === "0" || normalized === "200" || normalized === "2000" || normalized === "SUCCESS";
}
//# sourceMappingURL=index.js.map