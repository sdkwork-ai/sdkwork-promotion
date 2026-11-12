import {
  Button,
  DetailDrawer,
  DetailDrawerMetric,
  DetailDrawerMetrics,
  DetailDrawerSection,
} from "@sdkwork/ui-pc-react";
import type {
  SdkworkCouponCatalog,
  SdkworkUserCoupon,
} from "../coupon";
import { formatSdkworkCouponBenefit } from "../coupon";
import type { SdkworkCouponController } from "../coupon-controller";
import { useSdkworkCouponControllerState } from "../coupon-controller";
import {
  createSdkworkCouponMetricToneStyle,
  resolveSdkworkCouponStatusTone,
} from "../coupon-appearance";
import { useSdkworkCouponIntl } from "../coupon-intl";

export interface SdkworkCouponDetailDrawerProps {
  controller: SdkworkCouponController;
}

export function SdkworkCouponDetailDrawer({
  controller,
}: SdkworkCouponDetailDrawerProps) {
  const state = useSdkworkCouponControllerState(controller);
  const {
    copy,
    formatAvailability,
    formatCurrencyCny,
    formatPointCost,
    formatRemainingDays,
    formatStatus,
    formatTimestamp,
    formatType,
  } = useSdkworkCouponIntl();
  const detail = state.detail;
  const catalogDetail = state.detailKind === "catalog" && detail ? detail as SdkworkCouponCatalog : null;
  const ownedDetail = state.detailKind === "owned" && detail ? detail as SdkworkUserCoupon : null;
  const benefitLabel = detail
    ? formatSdkworkCouponBenefit(detail, copy.benefit)
    : null;
  const emptyValue = copy.common.emptyValue;
  const expireAtLabel = ownedDetail
    ? formatTimestamp(ownedDetail.expireAt)
    : catalogDetail
      ? formatTimestamp(catalogDetail.endTime)
      : emptyValue;
  const acquiredAtLabel = ownedDetail ? formatTimestamp(ownedDetail.acquireAt) : emptyValue;
  const stackableLabel = catalogDetail ? formatAvailability(catalogDetail.stackable) : emptyValue;
  const pointCostLabel = detail ? formatPointCost(detail.pointCost) : emptyValue;
  const availabilityLabel = ownedDetail
    ? formatAvailability(ownedDetail.available)
    : catalogDetail
      ? formatAvailability(catalogDetail.canReceive)
      : emptyValue;
  const remainingDaysLabel = ownedDetail ? formatRemainingDays(ownedDetail.remainingDays) : emptyValue;
  const orderIdLabel = ownedDetail?.orderId || emptyValue;
  const statusTone = detail ? resolveSdkworkCouponStatusTone(detail.status) : "default";
  const useAtLabel = ownedDetail ? formatTimestamp(ownedDetail.useAt) : emptyValue;

  return (
    <DetailDrawer
      description={detail?.name || copy.detail.summaryFallback}
      eyebrow={catalogDetail ? copy.detail.catalogEyebrow : ownedDetail ? copy.detail.ownedEyebrow : copy.detail.title}
      footer={(
        <div className="flex flex-wrap justify-end gap-3">
          {catalogDetail?.canReceive ? (
            <Button onClick={() => void controller.receiveCoupon(catalogDetail.id).catch(() => {})} type="button">
              {copy.actions.claimCoupon}
            </Button>
          ) : null}
          {catalogDetail?.pointsExchange ? (
            <Button onClick={() => void controller.exchangeCouponByPoints({ couponId: catalogDetail.id }).catch(() => {})} type="button" variant="outline">
              {copy.actions.exchangePoints}
            </Button>
          ) : null}
          {ownedDetail?.status === "used" ? (
            <Button
              onClick={() => void controller.cancelUseCoupon(ownedDetail.userCouponId ?? ownedDetail.id).catch(() => {})}
              type="button"
              variant="outline"
            >
              {copy.actions.cancelUse}
            </Button>
          ) : null}
          {ownedDetail && (ownedDetail.pointCost ?? 0) > 0 && !ownedDetail.pointsRefunded ? (
            <Button
              onClick={() => void controller.rollbackPointsExchange({ userCouponId: ownedDetail.userCouponId }).catch(() => {})}
              type="button"
              variant="outline"
            >
              {copy.actions.rollbackPoints}
            </Button>
          ) : null}
          <Button onClick={() => controller.closeDetail()} type="button" variant="ghost">
            {copy.actions.close}
          </Button>
        </div>
      )}
      onOpenChange={(open) => {
        if (!open) {
          controller.closeDetail();
        }
      }}
      open={state.isDetailOpen}
      summary={detail ? (
        <div className="flex flex-wrap items-center gap-3">
          <span
            className="rounded-full border px-3 py-1 text-xs font-semibold uppercase tracking-[0.14em]"
            data-sdk-tone={statusTone}
            style={createSdkworkCouponMetricToneStyle(statusTone)}
          >
            {formatStatus(detail.status)}
          </span>
          <span className="font-medium text-[var(--sdk-color-text-primary)]">
            {benefitLabel ?? (detail.amountCny != null && detail.amountCny > 0
              ? formatCurrencyCny(detail.amountCny)
              : copy.common.emptyValue)}
          </span>
          <span>{pointCostLabel}</span>
        </div>
      ) : copy.detail.loading}
      title={copy.detail.title}
    >
      {state.isDetailLoading || !detail ? (
        <div className="text-sm text-[var(--sdk-color-text-secondary)]">{copy.detail.loading}</div>
      ) : (
        <>
          <DetailDrawerMetrics columns={3}>
            <DetailDrawerMetric label={copy.common.coupon} value={detail.name} />
            <DetailDrawerMetric
              label={copy.detail.statusMetricLabel}
              tone={statusTone}
              value={formatStatus(detail.status)}
            />
            <DetailDrawerMetric
              label={copy.detail.discountMetricLabel}
              value={benefitLabel ?? (detail.amountCny != null && detail.amountCny > 0
                ? formatCurrencyCny(detail.amountCny)
                : copy.common.emptyValue)}
            />
            <DetailDrawerMetric label={copy.detail.pointCostLabel} value={pointCostLabel} />
          </DetailDrawerMetrics>

          <DetailDrawerSection description={copy.detail.overviewDescription} title={copy.detail.overviewTitle}>
            <div className="grid gap-3 text-sm text-[var(--sdk-color-text-secondary)] sm:grid-cols-2">
              <div>{copy.detail.typeLabel}: {formatType(detail.type)}</div>
              <div>{copy.detail.minimumSpendLabel}: {formatCurrencyCny(detail.minimumSpendCny)}</div>
              <div>{copy.detail.availabilityLabel}: {availabilityLabel}</div>
              <div>{copy.detail.remainingDaysLabel}: {remainingDaysLabel}</div>
              <div>{copy.detail.expireAtLabel}: {expireAtLabel}</div>
              <div>{copy.detail.acquiredAtLabel}: {acquiredAtLabel}</div>
            </div>
          </DetailDrawerSection>

          <DetailDrawerSection description={copy.detail.usageDescription} title={copy.detail.usageTitle}>
            <div className="grid gap-3 text-sm text-[var(--sdk-color-text-secondary)] sm:grid-cols-2">
              <div>{copy.detail.stackableLabel}: {stackableLabel}</div>
              <div>{copy.detail.orderIdLabel}: {orderIdLabel}</div>
              <div>{copy.detail.useAtLabel}: {useAtLabel}</div>
              <div>{copy.detail.couponIdLabel}: {"couponId" in detail ? detail.couponId || detail.id : detail.id}</div>
              <div>{copy.detail.userCouponIdLabel}: {"userCouponId" in detail ? detail.userCouponId || emptyValue : emptyValue}</div>
            </div>
          </DetailDrawerSection>
        </>
      )}
    </DetailDrawer>
  );
}
