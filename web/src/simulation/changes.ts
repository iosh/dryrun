import { formatAmount } from '../lib/formatting.ts';
import type { Asset, ChangeSet, Outcome, TokenMetadata } from './rpc.ts';

export function changeCount(changes: ChangeSet) {
  return changes.balances.length + changes.approvals.length + changes.delegations.length +
    (changes.protocol?.length ?? 0);
}

export function changesLabel(outcome: Outcome) {
  if (outcome.status !== 'success') return 'Not analyzed';
  if ('error' in outcome.changes) return 'Unavailable';
  const changes = outcome.changes;
  return `${changeCount(changes)} changes${changes.tokenReadFailures.length ? ' · token gaps' : ''}`;
}

export function tokenLabel(token: string, tokens: Record<string, TokenMetadata>) {
  const metadata = tokens[token];
  return metadata?.symbol || metadata?.name || 'Unknown token';
}

export function assetAmount(
  amount: string | bigint,
  asset: Asset,
  tokens: Record<string, TokenMetadata>,
  nativeSymbol: string,
) {
  switch (asset.type) {
    case 'native':
      return formatAmount(amount, 18, nativeSymbol);
    case 'erc20':
      return formatAmount(amount, tokens[asset.token]?.decimals, tokens[asset.token]?.symbol);
    case 'erc721':
    case 'erc1155':
      return formatAmount(amount, 0);
  }
}
