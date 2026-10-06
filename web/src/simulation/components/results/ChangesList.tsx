import { AlertTriangle } from 'lucide-react';

import { changeCount } from '../../changes.ts';
import { getEnvironment } from '../../environment.ts';
import type { SimulationRecord } from '../../types.ts';
import { AddressValue } from './AddressHighlight.tsx';
import { ApprovalChanges } from './ApprovalChanges.tsx';
import { BalanceChanges } from './BalanceChanges.tsx';
import { ProtocolChanges } from './ProtocolChanges.tsx';
import { ChangeBadge, ChangeSection, ResultShell } from './ResultPrimitives.tsx';
import type { AddressHighlightController } from './useAddressHighlight.ts';

export function ChangesList({
  record,
  addressHighlight,
}: Readonly<{ record: SimulationRecord; addressHighlight: AddressHighlightController }>) {
  const { outcome } = record.response;
  if (outcome.status !== 'success') {
    return <ResultShell><p className="text-sm text-ink-600">Changes are analyzed only after successful execution.</p></ResultShell>;
  }
  const { changes } = outcome;
  if ('error' in changes) {
    return (
      <section className="rounded-lg border border-amber-200 bg-amber-50 px-5 py-4 text-amber-950">
        <h3 className="flex items-center gap-2 text-sm font-semibold"><AlertTriangle aria-hidden="true" className="h-4 w-4" />Changes unavailable</h3>
        <p className="mt-2 text-xs leading-5">Execution succeeded, but change analysis failed. No change set is available.</p>
        <p className="mt-2 break-words text-sm">{changes.error.message}</p>
        <p className="mt-1 break-all font-mono text-[11px]">{changes.error.code}</p>
      </section>
    );
  }
  return (
    <>
      {changes.tokenReadFailures.length ? (
        <section className="rounded-lg border border-amber-200 bg-amber-50 px-5 py-4 text-amber-950">
          <h3 className="text-sm font-semibold">Some token changes could not be read</h3>
          <p className="mt-1 text-xs leading-5">The differences below were verified. Changes requiring the failed token reads are missing.</p>
          <ul className="mt-3 space-y-2">
            {changes.tokenReadFailures.map((failure, index) => (
              <li key={index}>
                <p className="font-mono text-[11px]">{failure.function}</p>
                <AddressValue address={failure.token} addressHighlight={addressHighlight} />
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      {changeCount(changes) === 0 ? (
        <ResultShell><p className="text-sm text-ink-600">No verified balance, approval, delegation or protocol differences were found.</p></ResultShell>
      ) : null}
      <BalanceChanges changes={changes} environment={getEnvironment(record.environmentId)} addressHighlight={addressHighlight} />
      <ApprovalChanges changes={changes} addressHighlight={addressHighlight} />
      {changes.protocol ? <ProtocolChanges changes={changes.protocol} addressHighlight={addressHighlight} /> : null}
      {changes.contracts.length ? (
        <ChangeSection title="Involved contracts" count={changes.contracts.length}>
          {changes.contracts.map((contract) => (
            <article className="px-5 py-4" key={contract.address}>
              <AddressValue address={contract.address} addressHighlight={addressHighlight} />
              <div className="mt-2 flex flex-wrap gap-2">
                {contract.called ? <ChangeBadge label="Called" tone="blue" /> : null}
                {contract.created ? <ChangeBadge label="Created" tone="green" /> : null}
                {contract.destroyed ? <ChangeBadge label="Destroyed" tone="red" /> : null}
                {contract.storageModified ? <ChangeBadge label="Storage modified" tone="violet" /> : null}
              </div>
            </article>
          ))}
        </ChangeSection>
      ) : null}
    </>
  );
}
