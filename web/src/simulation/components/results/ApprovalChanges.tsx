import { formatAmount, formatHexQuantity } from '../../../lib/formatting.ts';
import { tokenLabel } from '../../changes.ts';
import type { ChangeSet } from '../../rpc.ts';
import { AddressValue } from './AddressHighlight.tsx';
import { ChangeBadge, ChangeSection, StateDifference } from './ResultPrimitives.tsx';
import type { AddressHighlightController } from './useAddressHighlight.ts';

export function ApprovalChanges({
  changes,
  addressHighlight,
}: Readonly<{ changes: ChangeSet; addressHighlight: AddressHighlightController }>) {
  const address = (value: string | null) => value
    ? <AddressValue address={value} addressHighlight={addressHighlight} />
    : 'None';

  return (
    <>
      {changes.approvals.length ? (
        <ChangeSection title="Approvals" count={changes.approvals.length}>
          {changes.approvals.map((change, index) => {
            const metadata = changes.tokens[change.token];
            const allowance = (value: string) => formatAmount(value, metadata?.decimals, metadata?.symbol);
            return (
              <article className="px-5 py-4" key={index}>
                <div className="flex flex-wrap items-center gap-2">
                  <h4 className="break-all text-sm font-semibold">{tokenLabel(change.token, changes.tokens)}</h4>
                  <ChangeBadge
                    label={change.type === 'erc20' ? 'Allowance'
                      : change.type === 'erc721' ? `Token #${formatHexQuantity(change.id)}`
                        : 'Operator approval'}
                    tone="violet"
                  />
                </div>
                <p className="mt-2 text-[11px] text-ink-400">Token</p>
                {address(change.token)}
                {change.type !== 'erc721' ? (
                  <div className="mt-2 grid gap-2 sm:grid-cols-2">
                    <div className="min-w-0">
                      <p className="text-[11px] text-ink-400">Owner</p>
                      {address(change.owner)}
                    </div>
                    <div className="min-w-0">
                      <p className="text-[11px] text-ink-400">{change.type === 'erc20' ? 'Spender' : 'Operator'}</p>
                      {address(change.type === 'erc20' ? change.spender : change.operator)}
                    </div>
                  </div>
                ) : null}
                <StateDifference
                  before={change.type === 'erc20' ? allowance(change.before)
                    : change.type === 'erc721' ? address(change.before)
                      : change.before ? 'Approved' : 'Not approved'}
                  after={change.type === 'erc20' ? allowance(change.after)
                    : change.type === 'erc721' ? address(change.after)
                      : change.after ? 'Approved' : 'Not approved'}
                />
              </article>
            );
          })}
        </ChangeSection>
      ) : null}
      {changes.delegations.length ? (
        <ChangeSection title="Account delegations" count={changes.delegations.length} description="EIP-7702 delegation targets before and after execution.">
          {changes.delegations.map((change) => (
            <article className="px-5 py-4" key={change.account}>
              <p className="text-[11px] text-ink-400">Account</p>
              {address(change.account)}
              <StateDifference before={address(change.before)} after={address(change.after)} />
            </article>
          ))}
        </ChangeSection>
      ) : null}
    </>
  );
}
