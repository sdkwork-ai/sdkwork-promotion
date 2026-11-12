import { useEffect } from "react";
import {
  Button,
  LoadingBlock,
  StatusNotice,
} from "@sdkwork/ui-pc-react";
import type { SdkworkCouponMessagesOverrides } from "../coupon-copy";
import type { SdkworkCouponController } from "../coupon-controller";
import {
  useSdkworkCouponController,
  useSdkworkCouponControllerState,
} from "../coupon-controller";
import {
  SdkworkCouponIntlProvider,
  useSdkworkCouponIntl,
} from "../coupon-intl";
import { resolveSdkworkCouponStatusTone } from "../coupon-appearance";
import { formatSdkworkCouponBenefit, formatSdkworkUserCouponBenefit } from "../coupon";
import { SdkworkCouponDetailDrawer } from "../components/coupon-detail-drawer";
import { SdkworkCouponRedeemDialog } from "../components/coupon-redeem-dialog";

export interface SdkworkCouponPageProps {
  controller?: SdkworkCouponController;
  locale?: string | null;
  messages?: SdkworkCouponMessagesOverrides;
}

interface SdkworkCouponPageContentProps {
  controller?: SdkworkCouponController;
}

function SdkworkCouponPageContent({
  controller: controllerProp,
}: SdkworkCouponPageContentProps) {
  const {
    copy,
    formatCurrencyCny,
    locale,
    formatPointCost,
    formatRemainingDays,
    formatStatus,
  } = useSdkworkCouponIntl();
  const controller = useSdkworkCouponController(controllerProp, {
    locale,
    messages: copy,
  });
  const state = useSdkworkCouponControllerState(controller);

  useEffect(() => {
    if (!state.isBootstrapped && !state.isLoading && !state.lastError) {
      void controller.bootstrap().catch(() => undefined);
    }
  }, [controller, state.isBootstrapped, state.isLoading, state.lastError]);

  const stats = [
    {
      label: copy.stats.availableCoupons,
      value: state.dashboard.userDigest.availableCoupons,
    },
    {
      label: copy.stats.expiringSoon,
      value: state.dashboard.userDigest.expiringSoonCoupons,
    },
    {
      label: copy.page.highestDiscountLabel,
      value: formatCurrencyCny(state.dashboard.userDigest.highestDiscountAmountCny),
    },
    {
      label: copy.stats.claimableOffers,
      value: state.dashboard.catalogDigest.claimableCoupons,
    },
  ];

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="flex min-h-0 flex-1 flex-col px-4 py-4 sm:px-5 sm:py-5">
        <div className="mx-auto flex min-h-0 w-full max-w-5xl flex-1 flex-col gap-4">
          <section className="shrink-0 rounded-[var(--sdk-radius-panel)] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)]">
            <div className="flex flex-col gap-4 border-b border-[var(--sdk-color-border-subtle)] px-5 py-5 sm:flex-row sm:items-start sm:justify-between sm:px-6">
              <div>
                <h1 className="text-lg font-semibold tracking-tight text-[var(--sdk-color-text-primary)]">
                  {copy.page.title}
                </h1>
                <p className="mt-1 max-w-xl text-sm text-[var(--sdk-color-text-secondary)]">
                  {copy.page.description}
                </p>
              </div>
              <div className="flex shrink-0 flex-wrap gap-2">
                <Button onClick={() => controller.openRedeemDialog()} type="button">
                  {copy.actions.redeemCode}
                </Button>
                <Button onClick={() => void controller.refresh().catch(() => {})} type="button" variant="outline">
                  {copy.actions.refreshInventory}
                </Button>
              </div>
            </div>

            <dl className="grid grid-cols-2 divide-[var(--sdk-color-border-subtle)] sm:grid-cols-4 sm:divide-x">
              {stats.map((stat) => (
                <div className="px-5 py-4" key={stat.label}>
                  <dt className="text-xs text-[var(--sdk-color-text-muted)]">{stat.label}</dt>
                  <dd className="mt-1 text-lg font-semibold tabular-nums text-[var(--sdk-color-text-primary)]">
                    {stat.value}
                  </dd>
                </div>
              ))}
            </dl>
          </section>

          {state.isLoading && !state.isBootstrapped ? (
            <div className="shrink-0">
              <LoadingBlock label={copy.page.loading} />
            </div>
          ) : null}

          {state.lastError && !state.isMutating ? (
            <div className="shrink-0">
              <StatusNotice title={copy.page.errorTitle} tone="danger">
                {state.lastError}
              </StatusNotice>
            </div>
          ) : null}

          <section className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[var(--sdk-radius-panel)] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)]">
            <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-[var(--sdk-color-border-subtle)] px-5 py-4 sm:px-6">
              <div>
                <h2 className="text-sm font-semibold text-[var(--sdk-color-text-primary)]">
                  {copy.page.inventoryTitle}
                </h2>
              </div>

              <div className="flex flex-wrap gap-2">
                <Button onClick={() => controller.setTab("discover")} type="button" variant={state.activeTab === "discover" ? "secondary" : "outline"}>
                  {copy.actions.discover}
                </Button>
                <Button onClick={() => controller.setTab("my")} type="button" variant={state.activeTab === "my" ? "secondary" : "outline"}>
                  {copy.actions.myCoupons}
                </Button>
                <Button onClick={() => controller.setTab("history")} type="button" variant={state.activeTab === "history" ? "secondary" : "outline"}>
                  {copy.actions.history}
                </Button>
              </div>
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto">
              {state.activeTab === "discover" ? (
                state.visibleCatalogCoupons.length === 0 ? (
                  <div className="flex min-h-full items-center justify-center px-5 py-10 text-sm text-[var(--sdk-color-text-secondary)] sm:px-6">
                    {copy.inventory.emptyDiscover}
                  </div>
                ) : (
                  <div className="divide-y divide-[var(--sdk-color-border-subtle)]">
                    {state.visibleCatalogCoupons.map((coupon) => (
                      <article className="px-5 py-4 sm:px-6" key={coupon.id}>
                        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <h3 className="text-sm font-medium text-[var(--sdk-color-text-primary)]">{coupon.name}</h3>
                              <span
                                className="rounded-full border border-[var(--sdk-color-border-default)] px-2 py-0.5 text-xs text-[var(--sdk-color-text-secondary)]"
                                data-sdk-coupon-status={coupon.status}
                                data-sdk-tone={resolveSdkworkCouponStatusTone(coupon.status)}
                              >
                                {formatStatus(coupon.status)}
                              </span>
                            </div>
                            <p className="mt-1 text-sm text-[var(--sdk-color-text-secondary)]">
                              {coupon.description || copy.inventory.catalogFallbackDescription}
                            </p>
                            <p className="mt-2 text-lg font-semibold tabular-nums text-[var(--sdk-color-text-primary)]">
                              {formatSdkworkCouponBenefit(coupon, copy.benefit)
                                ?? (coupon.amountCny != null && coupon.amountCny > 0
                                  ? formatCurrencyCny(coupon.amountCny)
                                  : copy.common.emptyValue)}
                            </p>
                            <p className="mt-1 text-xs text-[var(--sdk-color-text-muted)]">
                              {copy.inventory.pointCostLabel}: {formatPointCost(coupon.pointCost)}
                            </p>
                          </div>
                          <div className="flex shrink-0 flex-wrap gap-2">
                            <Button onClick={() => void controller.openCatalogDetail(coupon.id).catch(() => {})} type="button" variant="outline">
                              {copy.actions.viewDetails}
                            </Button>
                            {coupon.canReceive ? (
                              <Button onClick={() => void controller.receiveCoupon(coupon.id).catch(() => {})} type="button">
                                {copy.actions.claimCoupon}
                              </Button>
                            ) : null}
                            {coupon.pointsExchange ? (
                              <Button onClick={() => void controller.exchangeCouponByPoints({ couponId: coupon.id }).catch(() => {})} type="button" variant="outline">
                                {copy.actions.exchangePoints}
                              </Button>
                            ) : null}
                          </div>
                        </div>
                      </article>
                    ))}
                  </div>
                )
              ) : (
                state.visibleUserCoupons.length === 0 ? (
                  <div className="flex min-h-full items-center justify-center px-5 py-10 text-sm text-[var(--sdk-color-text-secondary)] sm:px-6">
                    {copy.inventory.emptyVisible}
                  </div>
                ) : (
                  <div className="divide-y divide-[var(--sdk-color-border-subtle)]">
                    {state.visibleUserCoupons.map((coupon) => (
                      <article className="px-5 py-4 sm:px-6" key={coupon.id}>
                        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <h3 className="text-sm font-medium text-[var(--sdk-color-text-primary)]">{coupon.name}</h3>
                              <span
                                className="rounded-full border border-[var(--sdk-color-border-default)] px-2 py-0.5 text-xs text-[var(--sdk-color-text-secondary)]"
                                data-sdk-coupon-status={coupon.status}
                                data-sdk-tone={resolveSdkworkCouponStatusTone(coupon.status)}
                              >
                                {formatStatus(coupon.status)}
                              </span>
                            </div>
                            <p className="mt-1 text-sm text-[var(--sdk-color-text-secondary)]">
                              {copy.inventory.codeLabel}: {coupon.code || copy.common.emptyValue}
                            </p>
                            <p className="mt-2 text-lg font-semibold tabular-nums text-[var(--sdk-color-text-primary)]">
                              {formatSdkworkUserCouponBenefit(coupon, copy.benefit)
                                ?? (coupon.amountCny != null && coupon.amountCny > 0
                                  ? formatCurrencyCny(coupon.amountCny)
                                  : copy.common.emptyValue)}
                            </p>
                            <p className="mt-1 text-xs text-[var(--sdk-color-text-muted)]">
                              {copy.inventory.remainingDaysLabel}: {formatRemainingDays(coupon.remainingDays)}
                            </p>
                          </div>
                          <div className="flex shrink-0 gap-2">
                            <Button
                              onClick={() => void controller.openUserCouponDetail(coupon.userCouponId ?? coupon.id).catch(() => {})}
                              type="button"
                              variant="outline"
                            >
                              {copy.actions.viewDetails}
                            </Button>
                          </div>
                        </div>
                      </article>
                    ))}
                  </div>
                )
              )}
            </div>
          </section>
        </div>
      </div>

      <SdkworkCouponRedeemDialog controller={controller} />
      <SdkworkCouponDetailDrawer controller={controller} />
    </div>
  );
}

export function SdkworkCouponPage({
  locale,
  messages,
  ...props
}: SdkworkCouponPageProps) {
  const content = <SdkworkCouponPageContent {...props} />;

  if (locale || messages) {
    return (
      <SdkworkCouponIntlProvider locale={locale} messages={messages}>
        {content}
      </SdkworkCouponIntlProvider>
    );
  }

  return content;
}
