import { cn } from '../../../lib/cn.ts';
import { formatHexQuantity } from '../../../lib/formatting.ts';
import { assetAmount, tokenLabel } from '../../changes.ts';
import type { EnvironmentDefinition } from '../../environment.ts';
import type { ChangeSet } from '../../rpc.ts';
import { AddressValue } from './AddressHighlight.tsx';
import { ChangeBadge, ChangeSection, StateDifference } from './ResultPrimitives.tsx';
import type { AddressHighlightController } from './useAddressHighlight.ts';

export function BalanceChanges({
  changes,
  environment,
  addressHighlight,
}: Readonly<{
  changes: ChangeSet;
  environment: EnvironmentDefinition;
  addressHighlight: AddressHighlightController;
}>) {
  if (!changes.balances.length) return null;

  return (
    <ChangeSection
      count={changes.balances.length}
      description="Net balance differences for each holder. Native balances exclude gas fees."
      title="Balances"
    >
      {changes.balances.map((change, index) => {
        const { asset } = change;
        const delta = BigInt(change.after) - BigInt(change.before);
        const amount = (value: string | bigint) => assetAmount(value, asset, changes.tokens, environment.nativeSymbol);
        const space = environment.space === 'core-space'
          ? change.holder.startsWith('0x') ? 'eSpace' : 'Core Space'
          : null;
        return (
          <article className="px-5 py-4" key={index}>
            <div className="flex flex-wrap items-start justify-between gap-2">
              <div className="flex min-w-0 flex-wrap items-center gap-2">
                <h4 className="break-all text-sm font-semibold">
                  {asset.type === 'native' ? environment.nativeSymbol : tokenLabel(asset.token, changes.tokens)}
                  {'id' in asset ? ` #${formatHexQuantity(asset.id)}` : ''}
                </h4>
                <ChangeBadge label={asset.type === 'native' ? 'Native' : asset.type.toUpperCase()} tone="blue" />
                {space ? <span className="text-[11px] text-ink-400">{space}</span> : null}
              </div>
              <p className={cn('max-w-full break-all text-sm font-semibold', delta < 0n ? 'text-red-700' : 'text-emerald-700')}>
                {delta > 0n ? '+' : ''}{amount(delta)}
              </p>
            </div>
            {asset.type !== 'native' ? (
              <div className="mt-2">
                <p className="text-[11px] text-ink-400">Token</p>
                <AddressValue address={asset.token} addressHighlight={addressHighlight} />
              </div>
            ) : null}
            <div className="mt-2">
              <p className="text-[11px] text-ink-400">Holder</p>
              <AddressValue address={change.holder} addressHighlight={addressHighlight} />
            </div>
            <StateDifference before={amount(change.before)} after={amount(change.after)} />
          </article>
        );
      })}
    </ChangeSection>
  );
}
