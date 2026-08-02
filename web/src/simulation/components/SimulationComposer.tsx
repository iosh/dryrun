import { LoaderCircle, Play, RotateCcw } from 'lucide-react';

import { Button } from '../../ui/Button.tsx';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '../../ui/Tooltip.tsx';
import type { EnvironmentId } from '../environment.ts';
import type { SimulationFormApi } from '../form.ts';
import { AdvancedFields } from './composer/AdvancedFields.tsx';
import { FormIssues } from './composer/FormFields.tsx';
import { TransactionFields } from './composer/TransactionFields.tsx';

export interface SimulationComposerProps {
  environmentId: EnvironmentId;
  form: SimulationFormApi;
  isRunning: boolean;
  onReset: () => void;
}

export function SimulationComposer({
  environmentId,
  form,
  isRunning,
  onReset,
}: Readonly<SimulationComposerProps>) {
  return (
    <form
      className="mx-auto max-w-125"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <fieldset disabled={isRunning}>
        <div className="mb-6 flex items-start justify-between gap-4">
          <div>
            <p className="text-xs font-medium text-ink-600">Request</p>
            <h1 className="mt-1 text-2xl font-semibold">Transaction</h1>
          </div>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                aria-label="Reset transaction"
                onClick={onReset}
                size="icon"
                variant="secondary"
              >
                <RotateCcw aria-hidden="true" className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Reset transaction</TooltipContent>
          </Tooltip>
        </div>

        <TransactionFields environmentId={environmentId} form={form} />
        <AdvancedFields environmentId={environmentId} form={form} />
        <FormIssues form={form} />

        <Button className="mt-6 w-full gap-2" type="submit">
          {isRunning ? (
            <LoaderCircle
              aria-hidden="true"
              className="h-4.5 w-4.5 animate-spin"
            />
          ) : (
            <Play aria-hidden="true" className="h-4.5 w-4.5" />
          )}
          {isRunning ? 'Simulating' : 'Simulate'}
        </Button>
      </fieldset>
    </form>
  );
}
