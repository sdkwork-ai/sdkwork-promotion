import { appApiPath } from './paths';
import type { ApiRequestOptions, HttpClient } from '../http/client';

import type { NoData, PromotionsCodesRedemptionsCreateRequest, PromotionsCodesRedemptionsCreateResult, PromotionsMemberCardsConsumptionsCreateRequest } from '../types';


export class PromotionsPointsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List */
  async exchangeRules(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/wallet/points/exchanges/rules`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, skipAuth: true, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsWalletPointsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List */
  async balance(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/wallet/points`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }

/** List */
  async history(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/wallet/points/history`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsWalletApi {
  private client: HttpClient;
  public readonly points: PromotionsWalletPointsApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.points = new PromotionsWalletPointsApi(client);
  }


/** List */
  async exchangeRate(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/wallet/exchange_rate`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, skipAuth: true, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsMemberCardsConsumptionsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async create(cardId: string | number, body: PromotionsMemberCardsConsumptionsCreateRequest, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/member_cards/${serializePathParameter(cardId, { name: 'cardId', style: 'simple', explode: false })}/consumptions`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsMemberCardsApi {
  private client: HttpClient;
  public readonly consumptions: PromotionsMemberCardsConsumptionsApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.consumptions = new PromotionsMemberCardsConsumptionsApi(client);
  }


async list(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/member_cards`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }

async retrieve(cardId: string | number, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/member_cards/${serializePathParameter(cardId, { name: 'cardId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsUserCouponsWalletApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List */
  async list(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/user_coupons/wallet`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }

/** Retrieve */
  async retrieve(userCouponId: string, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/user_coupons/wallet/${serializePathParameter(userCouponId, { name: 'userCouponId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsUserCouponsClaimsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Create */
  async create(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/user_coupon_claims`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsUserCouponsApi {
  private client: HttpClient;
  public readonly claims: PromotionsUserCouponsClaimsApi;
  public readonly wallet: PromotionsUserCouponsWalletApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.claims = new PromotionsUserCouponsClaimsApi(client);
    this.wallet = new PromotionsUserCouponsWalletApi(client);
  }


/** List */
  async list(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/user_coupons`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }

/** Retrieve */
  async retrieve(userCouponId: string, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/user_coupons/${serializePathParameter(userCouponId, { name: 'userCouponId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsOffersApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List */
  async list(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/offers`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, skipAuth: true, sdkworkUnwrapKind: 'data' });
  }

/** Retrieve */
  async retrieve(offerId: string, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/offers/${serializePathParameter(offerId, { name: 'offerId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, skipAuth: true, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsDiscountApplicationsSettlementsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Settle */
  async create(applicationId: string, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/discount_applications/${serializePathParameter(applicationId, { name: 'applicationId', style: 'simple', explode: false })}/settlements`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsDiscountApplicationsReleasesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Release */
  async create(applicationId: string, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/discount_applications/${serializePathParameter(applicationId, { name: 'applicationId', style: 'simple', explode: false })}/releases`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsDiscountApplicationsReversalsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Create */
  async create(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/discount_applications/reversals`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsDiscountApplicationsApi {
  private client: HttpClient;
  public readonly reversals: PromotionsDiscountApplicationsReversalsApi;
  public readonly releases: PromotionsDiscountApplicationsReleasesApi;
  public readonly settlements: PromotionsDiscountApplicationsSettlementsApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.reversals = new PromotionsDiscountApplicationsReversalsApi(client);
    this.releases = new PromotionsDiscountApplicationsReleasesApi(client);
    this.settlements = new PromotionsDiscountApplicationsSettlementsApi(client);
  }


/** Create */
  async create(requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/discount_applications`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, sdkworkUnwrapKind: 'data' });
  }

/** Rollback */
  async rollback(applicationId: string, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/discount_applications/${serializePathParameter(applicationId, { name: 'applicationId', style: 'simple', explode: false })}/rollback`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsCodesRedemptionsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Create */
  async create(body: PromotionsCodesRedemptionsCreateRequest, requestOptions?: ApiRequestOptions): Promise<PromotionsCodesRedemptionsCreateResult> {
    return this.client.request<PromotionsCodesRedemptionsCreateResult>(appApiPath(`/promotions/codes/redemptions`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'data' });
  }

async preview(body: PromotionsCodesRedemptionsCreateRequest, requestOptions?: ApiRequestOptions): Promise<NoData> {
    return this.client.request<NoData>(appApiPath(`/promotions/codes/redemptions/preview`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'data' });
  }
}

export class PromotionsCodesApi {
  public readonly redemptions: PromotionsCodesRedemptionsApi;

  constructor(client: HttpClient) {
    this.redemptions = new PromotionsCodesRedemptionsApi(client);
  }

}

export class PromotionsApi {
  public readonly codes: PromotionsCodesApi;
  public readonly discountApplications: PromotionsDiscountApplicationsApi;
  public readonly offers: PromotionsOffersApi;
  public readonly userCoupons: PromotionsUserCouponsApi;
  public readonly memberCards: PromotionsMemberCardsApi;
  public readonly wallet: PromotionsWalletApi;
  public readonly points: PromotionsPointsApi;

  constructor(client: HttpClient) {
    this.codes = new PromotionsCodesApi(client);
    this.discountApplications = new PromotionsDiscountApplicationsApi(client);
    this.offers = new PromotionsOffersApi(client);
    this.userCoupons = new PromotionsUserCouponsApi(client);
    this.memberCards = new PromotionsMemberCardsApi(client);
    this.wallet = new PromotionsWalletApi(client);
    this.points = new PromotionsPointsApi(client);
  }

}

export function createPromotionsApi(client: HttpClient): PromotionsApi {
  return new PromotionsApi(client);
}



interface PathParameterSpec {
  name: string;
  style: string;
  explode: boolean;
}

function serializePathParameter(value: unknown, spec: PathParameterSpec): string {
  if (value === undefined || value === null) {
    return '';
  }

  const style = spec.style || 'simple';
  if (Array.isArray(value)) {
    return serializePathArray(spec.name, value, style, spec.explode);
  }
  if (typeof value === 'object') {
    return serializePathObject(spec.name, value as Record<string, unknown>, style, spec.explode);
  }
  return pathPrefix(spec.name, style, false) + encodePathValue(serializePathPrimitive(value));
}

function serializePathArray(name: string, values: unknown[], style: string, explode: boolean): string {
  const serialized = values
    .filter((item) => item !== undefined && item !== null)
    .map((item) => encodePathValue(serializePathPrimitive(item)));
  if (serialized.length === 0) {
    return pathPrefix(name, style, false);
  }
  if (style === 'matrix') {
    return explode
      ? serialized.map((item) => `;${name}=${item}`).join('')
      : `;${name}=${serialized.join(',')}`;
  }
  return pathPrefix(name, style, false) + serialized.join(explode ? '.' : ',');
}

function serializePathObject(name: string, value: Record<string, unknown>, style: string, explode: boolean): string {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (entries.length === 0) {
    return pathPrefix(name, style, true);
  }
  if (style === 'matrix') {
    return explode
      ? entries.map(([key, entryValue]) => `;${encodePathValue(key)}=${encodePathValue(serializePathPrimitive(entryValue))}`).join('')
      : `;${name}=${entries.flatMap(([key, entryValue]) => [encodePathValue(key), encodePathValue(serializePathPrimitive(entryValue))]).join(',')}`;
  }
  const serialized = explode
    ? entries.map(([key, entryValue]) => `${encodePathValue(key)}=${encodePathValue(serializePathPrimitive(entryValue))}`).join(style === 'label' ? '.' : ',')
    : entries.flatMap(([key, entryValue]) => [encodePathValue(key), encodePathValue(serializePathPrimitive(entryValue))]).join(',');
  return pathPrefix(name, style, true) + serialized;
}

function pathPrefix(name: string, style: string, _objectValue: boolean): string {
  if (style === 'label') return '.';
  if (style === 'matrix') return `;${name}`;
  return '';
}

function encodePathValue(value: string): string {
  return encodeURIComponent(value);
}

function serializePathPrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}
