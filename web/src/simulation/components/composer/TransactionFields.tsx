import { getEnvironment, type EnvironmentId } from '../../environment.ts';
import type { SimulationFormApi } from '../../form.ts';
import { TextAreaField, TextInputField } from './FormFields.tsx';

export function TransactionFields({
  environmentId,
  form,
}: Readonly<{
  environmentId: EnvironmentId;
  form: SimulationFormApi;
}>) {
  const environment = getEnvironment(environmentId);

  return (
    <div className="space-y-4">
      <TextInputField
        autoComplete="off"
        environmentId={environmentId}
        form={form}
        label="From"
        name="from"
        placeholder={environment.addressPlaceholder}
      />
      <TextInputField
        autoComplete="off"
        environmentId={environmentId}
        form={form}
        label="To"
        name="to"
        optional
        placeholder={environment.addressPlaceholder}
      />
      <TextInputField
        environmentId={environmentId}
        form={form}
        inputMode="decimal"
        label={`Value (${environment.nativeSymbol})`}
        name="value"
        optional
        placeholder="0.0"
      />
      <TextAreaField
        environmentId={environmentId}
        form={form}
        label="Data"
        monospace
        name="data"
        optional
        placeholder="0x"
        rows={4}
      />
    </div>
  );
}
