import { ChevronUp, FileJson2 } from 'lucide-react';

import { cn } from '../../lib/cn.ts';
import { Button } from '../../ui/Button.tsx';
import { LabeledField } from '../../ui/LabeledField.tsx';
import {
  inputChromeClassName,
  selectChromeClassName,
  textAreaChromeClassName,
} from '../../ui/fieldStyles.ts';
import { Panel } from '../../ui/Panel.tsx';
import {
  getSimulationFieldValidators,
  type SimulationFormApi,
} from '../form.ts';
import { formatSimulationRequestPreviewJson } from '../requestCodec.ts';
import type { SimulationFormValues, TxTypeOption } from '../types.ts';

export interface SimulationComposerProps {
  form: SimulationFormApi;
  isRunning: boolean;
  onReset: () => void;
}

export function SimulationComposer({
  form,
  isRunning,
  onReset,
}: Readonly<SimulationComposerProps>) {
  return (
    <Panel className="p-5 sm:p-6">
      <form
        className="space-y-6"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <div>
          <h1 className="font-display text-[28px] font-bold text-ink-950">
            Compose Transaction
          </h1>
        </div>

        <div className="space-y-4">
          <TextInputField form={form} label="From Address" name="from" placeholder="0x..." />
          <TextInputField
            form={form}
            label="To Address"
            name="to"
            optional
            placeholder="0x..."
          />

          <div className="grid gap-4 sm:grid-cols-2">
            <TextInputField
              form={form}
              label="Value (ETH)"
              name="valueEth"
              optional
              placeholder="0.0"
            />
            <TextInputField
              form={form}
              label="Gas Limit"
              name="gasLimit"
              optional
              placeholder="300000"
            />
          </div>

          <TextInputField
            form={form}
            label="Execution Block"
            name="executionBlock"
            optional
            placeholder="latest"
          />

          <TextAreaField
            form={form}
            label="Calldata"
            name="calldata"
            optional
            placeholder="0x"
          />
        </div>

        <div className="space-y-4 border-t border-line pt-4">
          <div className="flex items-center justify-between">
            <p className="font-mono text-[11px] font-semibold uppercase tracking-[0.16em] text-ink-600">
              Advanced Parameters
            </p>
            <ChevronUp className="text-ink-400" strokeWidth={2.25} />
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <TextInputField
              form={form}
              label="Chain ID"
              name="chainId"
              optional
              placeholder="Auto"
            />
            <TextInputField
              form={form}
              label="Nonce"
              name="nonce"
              optional
              placeholder="Auto"
            />
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <form.Field name="txType">
              {(field) => (
                <LabeledField label="Tx Type">
                  <select
                    className={selectChromeClassName}
                    onBlur={field.handleBlur}
                    onChange={(event) =>
                      field.handleChange(event.target.value as TxTypeOption)
                    }
                    value={field.state.value}
                  >
                    <option value="auto">Auto (Inferred)</option>
                    <option value="legacy">Legacy</option>
                    <option value="access-list">Access List</option>
                    <option value="dynamic-fee">Dynamic Fee</option>
                  </select>
                </LabeledField>
              )}
            </form.Field>
            <TextInputField
              form={form}
              label="Gas Price (Gwei)"
              name="gasPriceGwei"
              optional
              placeholder="Legacy / 2930"
            />
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <TextInputField
              form={form}
              label="Max Fee Per Gas (Gwei)"
              name="maxFeePerGasGwei"
              optional
              placeholder="1559 only"
            />
            <TextInputField
              form={form}
              label="Max Priority Fee (Gwei)"
              name="maxPriorityFeePerGasGwei"
              optional
              placeholder="1559 only"
            />
          </div>

          <div className="grid gap-4">
            <TextAreaField
              form={form}
              label="Access List (JSON)"
              name="accessListJson"
              optional
            />
          </div>
        </div>

        <FormIssuePanel form={form} />

        <div className="flex flex-col gap-3 sm:flex-row">
          <Button className="sm:flex-1" disabled={isRunning} type="submit">
            {isRunning ? 'Running…' : 'Run Simulation'}
          </Button>
          <Button
            disabled={isRunning}
            onClick={onReset}
            type="button"
            variant="secondary"
          >
            Reset
          </Button>
        </div>

        <div className="space-y-2 lg:hidden">
          <div className="flex items-center gap-2 font-mono text-[10px] font-semibold uppercase tracking-[0.16em] text-ink-600">
            <FileJson2 className="h-3.5 w-3.5" strokeWidth={2.25} />
            <p>JSON RPC Preview</p>
          </div>
          <Panel className="bg-shell-100 p-4 shadow-none">
            <RequestPreview form={form} />
          </Panel>
        </div>
      </form>
    </Panel>
  );
}

type StringFieldName = Exclude<keyof SimulationFormValues, 'txType'>;

interface TextInputFieldProps {
  form: SimulationFormApi;
  label: string;
  name: StringFieldName;
  optional?: boolean;
  placeholder?: string;
}

function TextInputField({
  form,
  label,
  name,
  optional = false,
  placeholder,
}: Readonly<TextInputFieldProps>) {
  return (
    <form.Field name={name} validators={getSimulationFieldValidators(name)}>
      {(field) => {
        const issues = normalizeIssues(field.state.meta.errors);
        const hasIssues = issues.length > 0;

        return (
          <LabeledField label={label} optional={optional}>
            <input
              aria-invalid={hasIssues}
              className={cn(
                inputChromeClassName,
                hasIssues &&
                  'border-red-300 focus:border-red-500 focus:ring-red-500/15',
              )}
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              placeholder={placeholder}
              value={field.state.value}
            />
            <FieldErrors
              errors={issues}
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
  form: SimulationFormApi;
  label: string;
  name: StringFieldName;
  optional?: boolean;
  placeholder?: string;
}

function TextAreaField({
  form,
  label,
  name,
  optional = false,
  placeholder,
}: Readonly<TextAreaFieldProps>) {
  return (
    <form.Field name={name} validators={getSimulationFieldValidators(name)}>
      {(field) => {
        const issues = normalizeIssues(field.state.meta.errors);
        const hasIssues = issues.length > 0;

        return (
          <LabeledField label={label} optional={optional}>
            <textarea
              aria-invalid={hasIssues}
              className={cn(
                textAreaChromeClassName,
                hasIssues &&
                  'border-red-300 focus:border-red-500 focus:ring-red-500/15',
              )}
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              placeholder={placeholder}
              value={field.state.value}
            />
            <FieldErrors
              errors={issues}
              form={form}
              isBlurred={field.state.meta.isBlurred}
            />
          </LabeledField>
        );
      }}
    </form.Field>
  );
}

function RequestPreview({ form }: Readonly<{ form: SimulationFormApi }>) {
  return (
    <form.Subscribe selector={(state) => state.values}>
      {(values) => (
        <pre className="overflow-x-auto font-mono text-[11px] leading-5 text-ink-950">
          {formatSimulationRequestPreviewJson(values)}
        </pre>
      )}
    </form.Subscribe>
  );
}

function FormIssuePanel({ form }: Readonly<{ form: SimulationFormApi }>) {
  return (
    <form.Subscribe
      selector={(state) => ({
        errors: state.errors,
        submissionAttempts: state.submissionAttempts,
      })}
    >
      {({ errors, submissionAttempts }) => {
        const issues = normalizeIssues(errors);

        if (submissionAttempts === 0 || issues.length === 0) {
          return null;
        }

        return (
          <Panel className="border-red-200 bg-red-50 p-4 shadow-none">
            <div className="space-y-2">
              <p className="text-sm font-semibold text-red-700">
                Fix these inputs before running a simulation
              </p>
              <ul className="space-y-1 text-sm text-red-700">
                {issues.map((issue) => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            </div>
          </Panel>
        );
      }}
    </form.Subscribe>
  );
}

function FieldErrors({
  errors,
  form,
  isBlurred,
}: Readonly<{
  errors: readonly string[];
  form: SimulationFormApi;
  isBlurred: boolean;
}>) {
  return (
    <form.Subscribe selector={(state) => state.submissionAttempts}>
      {(submissionAttempts) => {
        const shouldShow =
          errors.length > 0 && (isBlurred || submissionAttempts > 0);

        if (!shouldShow) {
          return null;
        }

        return <p className="text-sm text-red-700">{errors[0]}</p>;
      }}
    </form.Subscribe>
  );
}

function normalizeIssues(errors: readonly unknown[]) {
  return errors.flatMap(normalizeIssue);
}

function normalizeIssue(issue: unknown): string[] {
  if (typeof issue === 'string') {
    return [issue];
  }

  if (Array.isArray(issue)) {
    return issue.flatMap(normalizeIssue);
  }

  return [];
}
