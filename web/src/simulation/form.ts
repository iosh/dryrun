import { useForm } from '@tanstack/react-form';

import { INITIAL_FORM_VALUES } from './defaults.ts';
import {
  parseSimulationFormValues,
  validateSimulationField,
} from './requestCodec.ts';
import type { SimulationFormValues } from './types.ts';

type SubmitSimulationForm = (
  values: SimulationFormValues,
) => Promise<void> | void;

type HandleInvalidSimulationForm = () => void;

export function useSimulationForm(
  onSubmit: SubmitSimulationForm,
  onSubmitInvalid: HandleInvalidSimulationForm,
) {
  return useForm({
    defaultValues: INITIAL_FORM_VALUES,
    onSubmitInvalid,
    onSubmit: async ({ value }) => {
      await onSubmit(value);
    },
    validators: {
      onChange: ({ value }) =>
        toFormIssues(parseSimulationFormValues(value).formIssues),
      onSubmit: ({ value }) =>
        toFormIssues(parseSimulationFormValues(value).formIssues),
    },
  });
}

export type SimulationFormApi = ReturnType<typeof useSimulationForm>;

export function getSimulationFieldValidators<
  TKey extends keyof SimulationFormValues,
>(field: TKey) {
  return {
    onBlur: ({ value }: { value: SimulationFormValues[TKey] }) =>
      validateSimulationField(field, value),
    onChange: ({ value }: { value: SimulationFormValues[TKey] }) =>
      validateSimulationField(field, value),
    onSubmit: ({ value }: { value: SimulationFormValues[TKey] }) =>
      validateSimulationField(field, value),
  };
}

function toFormIssues(issues: readonly string[]) {
  if (issues.length === 0) {
    return undefined;
  }

  return {
    fields: {},
    form: issues,
  };
}
