import { backendApiPath } from './paths';
import type { ApiRequestOptions, HttpClient } from '../http/client';

import type { CouponStock, CouponStockRequest, DiscountApplication, PageInfo, PromotionCampaign, PromotionCampaignRequest, PromotionCode, PromotionCodeBatch, PromotionCodeBatchRequest, PromotionCouponLedgerEntry, PromotionDistributionRequest, PromotionDistributionTask, PromotionOffer, PromotionOfferRequest, PromotionOverview, PromotionUserCoupon, UpdatePromotionStatusRequest } from '../types';


export interface PromotionsCouponLedgerEntriesListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
}

export class PromotionsCouponLedgerEntriesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** couponLedgerEntries.list */
  async list(params?: PromotionsCouponLedgerEntriesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: PromotionCouponLedgerEntry[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: PromotionCouponLedgerEntry[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/coupon_ledger_entries`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface PromotionsUserCouponsListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
}

export class PromotionsUserCouponsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** userCoupons.list */
  async list(params?: PromotionsUserCouponsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: PromotionUserCoupon[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: PromotionUserCoupon[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/user_coupons`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface PromotionsDistributionTasksListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
}

export class PromotionsDistributionTasksApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** distributionTasks.list */
  async list(params?: PromotionsDistributionTasksListParams, requestOptions?: ApiRequestOptions): Promise<{ items: PromotionDistributionTask[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: PromotionDistributionTask[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/distribution_tasks`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** distributionTasks.create */
  async create(body: PromotionDistributionRequest, requestOptions?: ApiRequestOptions): Promise<PromotionDistributionTask> {
    return this.client.request<PromotionDistributionTask>(backendApiPath(`/promotions/distribution_tasks`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export interface PromotionsCodeBatchesListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
  stockId?: string;
}

export class PromotionsCodeBatchesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** codeBatches.list */
  async list(params?: PromotionsCodeBatchesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: PromotionCodeBatch[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
      { name: 'stock_id', value: params?.stockId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: PromotionCodeBatch[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/code_batches`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** codeBatches.create */
  async create(body: PromotionCodeBatchRequest, requestOptions?: ApiRequestOptions): Promise<PromotionCodeBatch> {
    return this.client.request<PromotionCodeBatch>(backendApiPath(`/promotions/code_batches`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export interface PromotionsCampaignsListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
}

export class PromotionsCampaignsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** campaigns.list */
  async list(params?: PromotionsCampaignsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: PromotionCampaign[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: PromotionCampaign[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/campaigns`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** campaigns.create */
  async create(body: PromotionCampaignRequest, requestOptions?: ApiRequestOptions): Promise<PromotionCampaign> {
    return this.client.request<PromotionCampaign>(backendApiPath(`/promotions/campaigns`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** campaigns.retrieve */
  async retrieve(campaignId: string, requestOptions?: ApiRequestOptions): Promise<PromotionCampaign> {
    return this.client.request<PromotionCampaign>(backendApiPath(`/promotions/campaigns/${serializePathParameter(campaignId, { name: 'campaignId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** campaigns.update */
  async update(campaignId: string, body: PromotionCampaignRequest, requestOptions?: ApiRequestOptions): Promise<PromotionCampaign> {
    return this.client.request<PromotionCampaign>(backendApiPath(`/promotions/campaigns/${serializePathParameter(campaignId, { name: 'campaignId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** campaigns.delete */
  async delete(campaignId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/promotions/campaigns/${serializePathParameter(campaignId, { name: 'campaignId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface PromotionsDiscountApplicationsListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
}

export class PromotionsDiscountApplicationsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List discount applications */
  async list(params?: PromotionsDiscountApplicationsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: DiscountApplication[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: DiscountApplication[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/discount_applications`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface PromotionsCodesListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
  codeBatchId?: string;
}

export class PromotionsCodesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List promotion codes */
  async list(params?: PromotionsCodesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: PromotionCode[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
      { name: 'code_batch_id', value: params?.codeBatchId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: PromotionCode[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/codes`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface PromotionsCouponStocksListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
}

export class PromotionsCouponStocksApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List coupon stock */
  async list(params?: PromotionsCouponStocksListParams, requestOptions?: ApiRequestOptions): Promise<{ items: CouponStock[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: CouponStock[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/coupon_stocks`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** couponStocks.create */
  async create(body: CouponStockRequest, requestOptions?: ApiRequestOptions): Promise<CouponStock> {
    return this.client.request<CouponStock>(backendApiPath(`/promotions/coupon_stocks`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export class PromotionsOffersStatusApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Enable or disable a promotion offer */
  async update(offerId: string, body: UpdatePromotionStatusRequest, requestOptions?: ApiRequestOptions): Promise<{ accepted: boolean; resourceId?: string; status?: string; }> {
    return this.client.request<{ accepted: boolean; resourceId?: string; status?: string; }>(backendApiPath(`/promotions/offers/${serializePathParameter(offerId, { name: 'offerId', style: 'simple', explode: false })}/status`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'command' });
  }
}

export interface PromotionsOffersListParams {
  page?: number;
  pageSize?: number;
  q?: string;
  status?: 'active' | 'disabled';
}

export class PromotionsOffersApi {
  private client: HttpClient;
  public readonly status: PromotionsOffersStatusApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.status = new PromotionsOffersStatusApi(client);
  }


/** List promotion offers */
  async list(params?: PromotionsOffersListParams, requestOptions?: ApiRequestOptions): Promise<{ items: PromotionOffer[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: PromotionOffer[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/promotions/offers`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** offers.create */
  async create(body: PromotionOfferRequest, requestOptions?: ApiRequestOptions): Promise<PromotionOffer> {
    return this.client.request<PromotionOffer>(backendApiPath(`/promotions/offers`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** offers.retrieve */
  async retrieve(offerId: string, requestOptions?: ApiRequestOptions): Promise<PromotionOffer> {
    return this.client.request<PromotionOffer>(backendApiPath(`/promotions/offers/${serializePathParameter(offerId, { name: 'offerId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** offers.update */
  async update(offerId: string, body: PromotionOfferRequest, requestOptions?: ApiRequestOptions): Promise<PromotionOffer> {
    return this.client.request<PromotionOffer>(backendApiPath(`/promotions/offers/${serializePathParameter(offerId, { name: 'offerId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** offers.delete */
  async delete(offerId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/promotions/offers/${serializePathParameter(offerId, { name: 'offerId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export class PromotionsOverviewApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Retrieve the promotion operations overview */
  async retrieve(requestOptions?: ApiRequestOptions): Promise<PromotionOverview> {
    return this.client.request<PromotionOverview>(backendApiPath(`/promotions/overview`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export class PromotionsApi {
  public readonly overview: PromotionsOverviewApi;
  public readonly offers: PromotionsOffersApi;
  public readonly couponStocks: PromotionsCouponStocksApi;
  public readonly codes: PromotionsCodesApi;
  public readonly discountApplications: PromotionsDiscountApplicationsApi;
  public readonly campaigns: PromotionsCampaignsApi;
  public readonly codeBatches: PromotionsCodeBatchesApi;
  public readonly distributionTasks: PromotionsDistributionTasksApi;
  public readonly userCoupons: PromotionsUserCouponsApi;
  public readonly couponLedgerEntries: PromotionsCouponLedgerEntriesApi;

  constructor(client: HttpClient) {
    this.overview = new PromotionsOverviewApi(client);
    this.offers = new PromotionsOffersApi(client);
    this.couponStocks = new PromotionsCouponStocksApi(client);
    this.codes = new PromotionsCodesApi(client);
    this.discountApplications = new PromotionsDiscountApplicationsApi(client);
    this.campaigns = new PromotionsCampaignsApi(client);
    this.codeBatches = new PromotionsCodeBatchesApi(client);
    this.distributionTasks = new PromotionsDistributionTasksApi(client);
    this.userCoupons = new PromotionsUserCouponsApi(client);
    this.couponLedgerEntries = new PromotionsCouponLedgerEntriesApi(client);
  }

}

export function createPromotionsApi(client: HttpClient): PromotionsApi {
  return new PromotionsApi(client);
}

function appendQueryString(path: string, rawQueryString: string): string {
  const query = rawQueryString.replace(/^\?+/, '');
  if (!query) {
    return path;
  }
  return path.includes('?') ? `${path}&${query}` : `${path}?${query}`;
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
interface QueryParameterSpec {
  name: string;
  value: unknown;
  style: string;
  explode: boolean;
  allowReserved: boolean;
  contentType?: string;
}

function buildQueryString(parameters: QueryParameterSpec[]): string {
  const pairs: string[] = [];
  for (const parameter of parameters) {
    appendSerializedParameter(pairs, parameter);
  }
  return pairs.join('&');
}

function appendSerializedParameter(pairs: string[], parameter: QueryParameterSpec): void {
  if (parameter.value === undefined || parameter.value === null) {
    return;
  }

  if (parameter.contentType) {
    pairs.push(`${encodeQueryComponent(parameter.name)}=${encodeQueryValue(JSON.stringify(parameter.value), parameter.allowReserved)}`);
    return;
  }

  const style = parameter.style || 'form';
  if (style === 'deepObject') {
    appendDeepObjectParameter(pairs, parameter.name, parameter.value, parameter.allowReserved);
    return;
  }

  if (Array.isArray(parameter.value)) {
    appendArrayParameter(pairs, parameter.name, parameter.value, style, parameter.explode, parameter.allowReserved);
    return;
  }

  if (typeof parameter.value === 'object') {
    appendObjectParameter(pairs, parameter.name, parameter.value as Record<string, unknown>, style, parameter.explode, parameter.allowReserved);
    return;
  }

  pairs.push(`${encodeQueryComponent(parameter.name)}=${encodeQueryValue(serializePrimitive(parameter.value), parameter.allowReserved)}`);
}

function appendArrayParameter(
  pairs: string[],
  name: string,
  value: unknown[],
  style: string,
  explode: boolean,
  allowReserved: boolean,
): void {
  const values = value
    .filter((item) => item !== undefined && item !== null)
    .map((item) => serializePrimitive(item));
  if (values.length === 0) {
    return;
  }

  if (style === 'form' && explode) {
    for (const item of values) {
      pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(item, allowReserved)}`);
    }
    return;
  }

  pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(values.join(','), allowReserved)}`);
}

function appendObjectParameter(
  pairs: string[],
  name: string,
  value: Record<string, unknown>,
  style: string,
  explode: boolean,
  allowReserved: boolean,
): void {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (entries.length === 0) {
    return;
  }

  if (style === 'form' && explode) {
    for (const [key, entryValue] of entries) {
      pairs.push(`${encodeQueryComponent(key)}=${encodeQueryValue(serializePrimitive(entryValue), allowReserved)}`);
    }
    return;
  }

  const serialized = entries.flatMap(([key, entryValue]) => [key, serializePrimitive(entryValue)]).join(',');
  pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(serialized, allowReserved)}`);
}

function appendDeepObjectParameter(
  pairs: string[],
  name: string,
  value: unknown,
  allowReserved: boolean,
): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(serializePrimitive(value), allowReserved)}`);
    return;
  }

  for (const [key, entryValue] of Object.entries(value as Record<string, unknown>)) {
    if (entryValue === undefined || entryValue === null) {
      continue;
    }
    pairs.push(`${encodeQueryComponent(`${name}[${key}]`)}=${encodeQueryValue(serializePrimitive(entryValue), allowReserved)}`);
  }
}

function serializePrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

function encodeQueryComponent(value: string): string {
  return encodeURIComponent(value);
}

function encodeQueryValue(value: string, allowReserved: boolean): string {
  const encoded = encodeURIComponent(value);
  if (!allowReserved) {
    return encoded;
  }
  return encoded.replace(/%3A/gi, ':')
    .replace(/%2F/gi, '/')
    .replace(/%3F/gi, '?')
    .replace(/%23/gi, '#')
    .replace(/%5B/gi, '[')
    .replace(/%5D/gi, ']')
    .replace(/%40/gi, '@')
    .replace(/%21/gi, '!')
    .replace(/%24/gi, '$')
    .replace(/%26/gi, '&')
    .replace(/%27/gi, "'")
    .replace(/%28/gi, '(')
    .replace(/%29/gi, ')')
    .replace(/%2A/gi, '*')
    .replace(/%2B/gi, '+')
    .replace(/%2C/gi, ',')
    .replace(/%3B/gi, ';')
    .replace(/%3D/gi, '=');
}
