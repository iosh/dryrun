import type { ReactNode } from 'react';

import { formatHexQuantity, formatNativeAmount } from '../../../lib/formatting.ts';
import type { ProtocolChange } from '../../rpc.ts';
import { AddressValue } from './AddressHighlight.tsx';
import { ChangeSection, StateDifference } from './ResultPrimitives.tsx';
import type { AddressHighlightController } from './useAddressHighlight.ts';

const LABELS: Record<ProtocolChange['type'], string> = {
  stakingBalance: 'Staking balance',
  accumulatedInterestReturn: 'Accumulated interest return',
  collateralForStorage: 'Storage collateral',
  depositList: 'Staking deposits',
  voteStakeList: 'Vote locks',
  posIdentifier: 'PoS identifier',
  posStake: 'PoS stake',
  governanceVotes: 'Governance votes',
  admin: 'Contract admin',
  gasSponsor: 'Gas sponsor',
  storageSponsor: 'Storage sponsor',
  sponsorWhitelist: 'Sponsor whitelist',
};

export function ProtocolChanges({
  changes,
  addressHighlight,
}: Readonly<{ changes: ProtocolChange[]; addressHighlight: AddressHighlightController }>) {
  if (!changes.length) return null;
  return (
    <ChangeSection title="Core protocol state" count={changes.length} description="Sponsor pool balances include any fees paid by the pool.">
      {changes.map((change, index) => {
        const target = 'account' in change ? change.account
          : 'contract' in change ? change.contract
            : 'voter' in change ? change.voter : null;
        return (
          <article className="px-5 py-4" key={index}>
            <h4 className="mb-2 text-sm font-semibold">{LABELS[change.type]}</h4>
            {target ? <AddressValue address={target} addressHighlight={addressHighlight} /> : null}
            {change.type === 'posStake' ? <p className="break-all font-mono text-[11px] text-ink-600">{change.identifier}</p> : null}
            {change.type === 'sponsorWhitelist' ? (
              <div className="mt-2">
                <p className="text-[11px] text-ink-400">User</p>
                {change.user ? <AddressValue address={change.user} addressHighlight={addressHighlight} /> : <p className="text-xs leading-6">All users (public entry)</p>}
              </div>
            ) : null}
            <StateDifference
              before={<ProtocolState change={change} side="before" addressHighlight={addressHighlight} />}
              after={<ProtocolState change={change} side="after" addressHighlight={addressHighlight} />}
            />
          </article>
        );
      })}
    </ChangeSection>
  );
}

function ProtocolState({
  change,
  side,
  addressHighlight,
}: Readonly<{ change: ProtocolChange; side: 'before' | 'after'; addressHighlight: AddressHighlightController }>) {
  const address = (value: string | null) => value
    ? <AddressValue address={value} addressHighlight={addressHighlight} /> : 'None';
  const cfx = (value: string) => formatNativeAmount(value, 'CFX');

  switch (change.type) {
    case 'stakingBalance':
    case 'accumulatedInterestReturn':
    case 'collateralForStorage':
      return cfx(change[side]);
    case 'posIdentifier':
      return <span className="break-all font-mono text-[11px]">{change[side] ?? 'None'}</span>;
    case 'admin':
      return address(change[side]);
    case 'sponsorWhitelist':
      return change[side] ? 'Enabled' : 'Disabled';
    case 'depositList':
      return change[side].length ? (
        <ul className="space-y-3">
          {change[side].map((deposit, index) => (
            <li key={index}><StateValues values={[
              ['Amount', cfx(deposit.amount)],
              ['Deposit block', formatHexQuantity(deposit.depositTime)],
              ['Accumulated rate (raw)', formatHexQuantity(deposit.accumulatedInterestRate)],
            ]} /></li>
          ))}
        </ul>
      ) : 'No deposits';
    case 'voteStakeList':
      return change[side].length ? (
        <ul className="space-y-3">
          {change[side].map((lock, index) => (
            <li key={index}><StateValues values={[
              ['Locked', cfx(lock.amount)],
              ['Unlock block', formatHexQuantity(lock.unlockBlockNumber)],
            ]} /></li>
          ))}
        </ul>
      ) : 'No vote locks';
    case 'posStake': {
      const state = change[side];
      return <StateValues values={[
        ['Account', address(state.account)],
        ['Registered', formatHexQuantity(state.registered)],
        ['Unlocked', formatHexQuantity(state.unlocked)],
      ]} />;
    }
    case 'governanceVotes':
      return change[side].length ? (
        <ul className="space-y-3">
          {change[side].map((vote) => (
            <li key={vote.index}><StateValues values={[
              ['Parameter index', formatHexQuantity(vote.index)],
              ['Votes [0, 1, 2]', vote.votes.map(formatHexQuantity).join(' / ')],
            ]} /></li>
          ))}
        </ul>
      ) : 'No votes';
    case 'gasSponsor': {
      const state = change[side];
      return <StateValues values={[
        ['Sponsor', address(state.sponsor)],
        ['Pool balance', cfx(state.balance)],
        ['Sponsorship bound', cfx(state.gasBound)],
      ]} />;
    }
    case 'storageSponsor': {
      const state = change[side];
      return <StateValues values={[
        ['Sponsor', address(state.sponsor)],
        ['Pool balance', cfx(state.balance)],
        ['Storage points', state.storagePoints
          ? <StateValues values={[
            ['Unused', formatHexQuantity(state.storagePoints.unused)],
            ['Used', formatHexQuantity(state.storagePoints.used)],
          ]} />
          : 'None'],
      ]} />;
    }
  }
}

function StateValues({ values }: Readonly<{ values: [string, ReactNode][] }>) {
  return (
    <dl className="space-y-1">
      {values.map(([label, value]) => (
        <div className="min-w-0" key={label}>
          <dt className="text-[10px] text-ink-400">{label}</dt>
          <dd className="break-all">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
