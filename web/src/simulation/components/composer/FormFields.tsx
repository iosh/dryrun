import { cn } from '../../../lib/cn.ts';
import { Input } from '../../../ui/Input.tsx';
import { LabeledField } from '../../../ui/LabeledField.tsx';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../../ui/Select.tsx';
import { Textarea } from '../../../ui/Textarea.tsx';
import type { EnvironmentId } from '../../environment.ts';
import {
  getSimulationFieldValidators,
  type SimulationFormApi,
} from '../../form.ts';
import type {
  ContextMode,
  SimulationFormValues,
  TxTypeOption,
} from '../../types.ts';

export type StringFieldName = Exclude<
  keyof SimulationFormValues,
  'contextMode' | 'txType'
>;

interface TextInputFieldProps {
  autoComplete?: string;
  disabled?: boolean;
  environmentId: EnvironmentId;
  form: SimulationFormApi;
  inputMode?: 'decimal' | 'numeric' | 'text';
  label: string;
  name: StringFieldName;
  optional?: boolean;
  placeholder?: string;
}

export function TextInputField({
  autoComplete,
  disabled = false,
  environmentId,
  form,
  inputMode,
  label,
  name,
  optional = false,
  placeholder,
}: Readonly<TextInputFieldProps>) {
  return (
    <form.Field
      name={name}
      validators={getSimulationFieldValidators(environmentId, name)}
    >
      {(field) => {
        const issues = collectIssueMessages(field.state.meta.errors);
        return (
          <LabeledField label={label} optional={optional}>
            <Input
              aria-invalid={issues.length > 0}
              autoComplete={autoComplete}
              disabled={disabled}
              inputMode={inputMode}
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              placeholder={placeholder}
              spellCheck={false}
              value={field.state.value}
            />
            <FieldIssues
              fieldIssues={issues}
              form={form}
              isBlurred={field.state.meta.isBlurred}
            />
          </LabeledField>
        );
      }}
    </form.Field>
  );
}

interface TextAreaFieldProps {
  disabled?: boolean;
  environmentId: EnvironmentId;
  form: SimulationFormApi;
  label: string;
  monospace?: boolean;
  name: StringFieldName;
  optional?: boolean;
  placeholder?: string;
  rows: number;
}

export function TextAreaField({
  disabled = false,
  environmentId,
  form,
  label,
  monospace = false,
  name,
  optional = false,
  placeholder,
  rows,
}: Readonly<TextAreaFieldProps>) {
  return (
    <form.Field
      name={name}
      validators={getSimulationFieldValidators(environmentId, name)}
    >
      {(field) => {
        const issues = collectIssueMessages(field.state.meta.errors);
        return (
          <LabeledField label={label} optional={optional}>
            <Textarea
              aria-invalid={issues.length > 0}
              className={cn(
                monospace && 'font-mono text-xs',
              )}
              disabled={disabled}
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              placeholder={placeholder}
              rows={rows}
              spellCheck={false}
              value={field.state.value}
            />
            <FieldIssues
              fieldIssues={issues}
              form={form}
              isBlurred={field.state.meta.isBlurred}
            />
          </LabeledField>
        );
      }}
    </form.Field>
  );
}

export function ContextModeField({
  environmentId,
  form,
}: Readonly<{
  environmentId: EnvironmentId;
  form: SimulationFormApi;
}>) {
  return (
    <form.Field name="contextMode">
      {(field) => (
        <LabeledField label="State context">
          <Select
            onValueChange={(value) => field.handleChange(value as ContextMode)}
            value={field.state.value}
          >
            <SelectTrigger aria-label="State context" onBlur={field.handleBlur}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="latest">Latest</SelectItem>
              {environmentId === 'ethereum-mainnet' ? (
                <>
                  <SelectItem value="safe">Safe</SelectItem>
                  <SelectItem value="finalized">Finalized</SelectItem>
                </>
              ) : null}
              {environmentId === 'conflux-espace-mainnet' ? (
                <SelectItem value="hash">Hash</SelectItem>
              ) : null}
              <SelectItem value="number">Number</SelectItem>
            </SelectContent>
          </Select>
        </LabeledField>
      )}
    </form.Field>
  );
}

export function TxTypeField({ form }: Readonly<{ form: SimulationFormApi }>) {
  return (
    <form.Field name="txType">
      {(field) => (
        <LabeledField label="Transaction type">
          <Select
            onValueChange={(value) => {
              const transactionType = value as TxTypeOption;
              field.handleChange(transactionType);
              clearIncompatibleTransactionFields(form, transactionType);
            }}
            value={field.state.value}
          >
            <SelectTrigger aria-label="Transaction type" onBlur={field.handleBlur}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="auto">Auto</SelectItem>
              <SelectItem value="legacy">Legacy</SelectItem>
              <SelectItem value="access-list">Access list</SelectItem>
              <SelectItem value="dynamic-fee">Dynamic fee</SelectItem>
            </SelectContent>
          </Select>
        </LabeledField>
      )}
    </form.Field>
  );
}

function clearIncompatibleTransactionFields(
  form: SimulationFormApi,
  transactionType: TxTypeOption,
) {
  const clearField = (
    field:
      | 'accessListJson'
      | 'gasPrice'
      | 'maxFeePerGas'
      | 'maxPriorityFeePerGas',
  ) => form.setFieldValue(field, '', { dontUpdateMeta: true });

  switch (transactionType) {
    case 'legacy':
      clearField('accessListJson');
      clearField('maxFeePerGas');
      clearField('maxPriorityFeePerGas');
      break;
    case 'access-list':
      clearField('maxFeePerGas');
      clearField('maxPriorityFeePerGas');
      break;
    case 'dynamic-fee':
      clearField('gasPrice');
      break;
    case 'auto':
      break;
  }
}

export function AutoField({ label }: Readonly<{ label: string }>) {
  return (
    <LabeledField label={label}>
      <div className="flex h-11 items-center rounded-md border border-line bg-shell-100 px-3 text-sm text-ink-400">
        Auto
      </div>
    </LabeledField>
  );
}

export function FormIssues({ form }: Readonly<{ form: SimulationFormApi }>) {
  return (
    <form.Subscribe
      selector={(state) => ({
        errors: state.errors,
        submissionAttempts: state.submissionAttempts,
      })}
    >
      {({ errors, submissionAttempts }) => {
        const issues = collectIssueMessages(errors);
        if (submissionAttempts === 0 || issues.length === 0) return null;

        return (
          <div className="mt-5 border-l-2 border-red-500 bg-red-50 px-4 py-3 text-sm text-red-800">
            {issues.map((issue) => (
              <p key={issue}>{issue}</p>
            ))}
          </div>
        );
      }}
    </form.Subscribe>
  );
}

function FieldIssues({
  fieldIssues,
  form,
  isBlurred,
}: Readonly<{
  fieldIssues: readonly string[];
  form: SimulationFormApi;
  isBlurred: boolean;
}>) {
  return (
    <form.Subscribe selector={(state) => state.submissionAttempts}>
      {(submissionAttempts) =>
        fieldIssues.length > 0 && (isBlurred || submissionAttempts > 0) ? (
          <p className="text-xs leading-5 text-red-700">{fieldIssues[0]}</p>
        ) : null
      }
    </form.Subscribe>
  );
}

function collectIssueMessages(errors: readonly unknown[]) {
  return errors.flatMap((error): string[] => {
    if (typeof error === 'string') return [error];
    if (Array.isArray(error)) return collectIssueMessages(error);
    return [];
  });
}
