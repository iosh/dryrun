import { ChevronDown, SlidersHorizontal } from 'lucide-react';

import { getEnvironment, type EnvironmentId } from '../../environment.ts';
import type { SimulationFormApi } from '../../form.ts';
import { countAdvancedValues } from '../../request.ts';
import {
  AutoField,
  ContextModeField,
  TextAreaField,
  TextInputField,
  TxTypeField,
} from './FormFields.tsx';

export function AdvancedFields({
  environmentId,
  form,
}: Readonly<{
  environmentId: EnvironmentId;
  form: SimulationFormApi;
}>) {
  const environment = getEnvironment(environmentId);

  return (
    <form.Subscribe selector={(state) => state.values}>
      {(values) => {
        const advancedCount = countAdvancedValues(environmentId, values);
        const contextLabel =
          values.contextMode === 'hash'
            ? 'Block hash'
            : environment.contextKind === 'block'
              ? 'Block number'
              : 'Epoch number';
        const hasGasPrice = values.gasPrice.trim().length > 0;
        const hasDynamicFee =
          values.maxFeePerGas.trim().length > 0 ||
          values.maxPriorityFeePerGas.trim().length > 0;
        const hasAuthorization =
          values.authorizationListJson.trim().length > 0 &&
          values.authorizationListJson.trim() !== '[]';
        const gasPriceDisabled =
          values.txType === 'dynamic-fee' ||
          values.txType === 'eip7702' ||
          (values.txType === 'auto' &&
            !hasGasPrice &&
            (hasDynamicFee || hasAuthorization));
        const dynamicFeeDisabled =
          values.txType === 'legacy' ||
          values.txType === 'access-list' ||
          (values.txType === 'auto' && hasGasPrice);
        const accessListDisabled = values.txType === 'legacy';

        return (
          <details className="group mt-6 border-y border-line py-1">
            <summary className="flex min-h-12 cursor-pointer list-none items-center justify-between gap-3 rounded-md px-1 text-sm font-medium text-ink-950 outline-none focus-visible:ring-2 focus-visible:ring-brand-600/20 [&::-webkit-details-marker]:hidden">
              <span className="flex items-center gap-2">
                <SlidersHorizontal
                  aria-hidden="true"
                  className="h-4 w-4 text-ink-600"
                />
                Advanced
                {advancedCount > 0 ? (
                  <span className="flex h-5 min-w-5 items-center justify-center rounded-full bg-brand-50 px-1.5 text-[11px] font-semibold text-brand-700">
                    {advancedCount}
                  </span>
                ) : null}
              </span>
              <ChevronDown
                aria-hidden="true"
                className="h-4 w-4 text-ink-600 transition-transform group-open:rotate-180"
              />
            </summary>

            <div className="space-y-4 px-1 pb-5 pt-3">
              <div className="grid gap-4 sm:grid-cols-2">
                <ContextModeField
                  environmentId={environmentId}
                  form={form}
                />
                {values.contextMode === 'number' || values.contextMode === 'hash' ? (
                  <TextInputField
                    environmentId={environmentId}
                    form={form}
                    inputMode={values.contextMode === 'hash' ? 'text' : 'numeric'}
                    label={contextLabel}
                    name="contextNumber"
                  />
                ) : (
                  <AutoField label={contextLabel} />
                )}
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <TextInputField
                  environmentId={environmentId}
                  form={form}
                  inputMode="numeric"
                  label="Nonce"
                  name="nonce"
                  optional
                  placeholder="Auto"
                />
                <TextInputField
                  environmentId={environmentId}
                  form={form}
                  inputMode="numeric"
                  label="Gas limit"
                  name="gasLimit"
                  optional
                  placeholder="Auto"
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <TxTypeField environmentId={environmentId} form={form} />
                <TextInputField
                  disabled={gasPriceDisabled}
                  environmentId={environmentId}
                  form={form}
                  inputMode="decimal"
                  label={`Gas price (${environment.feeUnit})`}
                  name="gasPrice"
                  optional
                  placeholder="Auto"
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <TextInputField
                  disabled={dynamicFeeDisabled}
                  environmentId={environmentId}
                  form={form}
                  inputMode="decimal"
                  label={`Max fee (${environment.feeUnit})`}
                  name="maxFeePerGas"
                  optional
                  placeholder="Auto"
                />
                <TextInputField
                  disabled={dynamicFeeDisabled}
                  environmentId={environmentId}
                  form={form}
                  inputMode="decimal"
                  label={`Priority fee (${environment.feeUnit})`}
                  name="maxPriorityFeePerGas"
                  optional
                  placeholder="Auto"
                />
              </div>

              {environmentId === 'conflux-core-mainnet' ? (
                <div className="grid gap-4 sm:grid-cols-2">
                  <TextInputField
                    environmentId={environmentId}
                    form={form}
                    inputMode="numeric"
                    label="Storage limit"
                    name="storageLimit"
                    optional
                    placeholder="Auto"
                  />
                  <TextInputField
                    environmentId={environmentId}
                    form={form}
                    inputMode="numeric"
                    label="Epoch height"
                    name="epochHeight"
                    optional
                    placeholder="Auto"
                  />
                </div>
              ) : null}

              <TextAreaField
                disabled={accessListDisabled}
                environmentId={environmentId}
                form={form}
                label="Access list (JSON)"
                monospace
                name="accessListJson"
                optional
                placeholder="[]"
                rows={5}
              />

              {environmentId === 'conflux-espace-mainnet' ? (
                <TextAreaField
                  disabled={
                    values.txType !== 'auto' && values.txType !== 'eip7702'
                  }
                  environmentId={environmentId}
                  form={form}
                  label="Signed authorizations (JSON)"
                  monospace
                  name="authorizationListJson"
                  optional
                  placeholder="[]"
                  rows={6}
                />
              ) : null}
            </div>
          </details>
        );
      }}
    </form.Subscribe>
  );
}
