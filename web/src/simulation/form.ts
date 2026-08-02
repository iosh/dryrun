import { useForm } from '@tanstack/react-form';

import type { EnvironmentId } from './environment.ts';
import {
  createInitialFormValues,
  parseSimulationForm,
  validateSimulationField,
} from './request.ts';
import type { SimulationFormValues } from './types.ts';

type SubmitSimulationForm = (
  values: SimulationFormValues,
) => Promise<void> | void;

export function useSimulationForm(
  environmentId: EnvironmentId,
  onSubmit: SubmitSimulationForm,
) {
  return useForm({
    defaultValues: createInitialFormValues(),
    onSubmit: async ({ value }) => {
      await onSubmit(value);
    },
    validators: {
      onSubmit: ({ value }) => {
        const { fieldIssues, formIssues } = parseSimulationForm(
          environmentId,
          value,
        );
        if (
          Object.keys(fieldIssues).length === 0 &&
          formIssues.length === 0
        ) {
          return undefined;
        }

        return {
          fields: fieldIssues,
          form: formIssues,
        };
      },
    },
  });
}

export type SimulationFormApi = ReturnType<typeof useSimulationForm>;

export function getSimulationFieldValidators<
  TKey extends keyof SimulationFormValues,
>(environmentId: EnvironmentId, field: TKey) {
  const validate = ({ value }: { value: SimulationFormValues[TKey] }) =>
    validateSimulationField(environmentId, field, value);

  return {
    onBlur: validate,
    onSubmit: validate,
  };
}
