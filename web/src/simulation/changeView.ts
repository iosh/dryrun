import { formatUnits } from 'viem';

import { shortAddress } from '../lib/formatting.ts';
import type {
  DryrunAsset,
  DryrunChange,
  DryrunCollection,
} from './rpc.ts';

export interface ChangeItemViewModel {
  badgeLabel: string;
  badgeTone: 'amber' | 'blue' | 'green' | 'slate' | 'violet';
  description: string;
  title: string;
  value?: string;
}

export function toChangeItemViewModel(change: DryrunChange) {
  switch (change.kind) {
    case 'TRANSFER':
      return {
        badgeLabel: 'Transfer',
        badgeTone: 'blue',
        description: `Source ${shortAddress(change.from)} → Destination ${shortAddress(change.to)}`,
        title: assetTitle(change.asset),
        value: changeValue(change.asset, change.amount),
      } satisfies ChangeItemViewModel;
    case 'MINT':
      return {
        badgeLabel: 'Mint',
        badgeTone: 'violet',
        description: `Minted to ${shortAddress(change.to)}`,
        title: assetTitle(change.asset),
        value: changeValue(change.asset, change.amount),
      } satisfies ChangeItemViewModel;
    case 'BURN':
      return {
        badgeLabel: 'Burn',
        badgeTone: 'amber',
        description: `Burned from ${shortAddress(change.from)}`,
        title: assetTitle(change.asset),
        value: changeValue(change.asset, change.amount),
      } satisfies ChangeItemViewModel;
    case 'APPROVAL':
      return {
        badgeLabel: 'Approval',
        badgeTone: 'green',
        description: `Owner ${shortAddress(change.owner)} approved ${shortAddress(change.spender)}`,
        title: assetTitle(change.asset),
        value: changeValue(change.asset, change.amount),
      } satisfies ChangeItemViewModel;
    case 'APPROVAL_FOR_ALL':
      return {
        badgeLabel: 'Approval For All',
        badgeTone: 'green',
        description: `Operator ${shortAddress(change.operator)} for ${shortAddress(change.owner)}`,
        title: collectionTitle(change.collection),
        value: change.approved ? 'Enabled' : 'Revoked',
      } satisfies ChangeItemViewModel;
  }
}

function assetTitle(asset: DryrunAsset) {
  switch (asset.type) {
    case 'NATIVE':
      return asset.display?.symbol ?? 'Native Asset';
    case 'ERC20':
      return (
        asset.display?.symbol ??
        asset.display?.name ??
        shortAddress(asset.contractAddress)
      );
    case 'ERC721':
      return (
        asset.token?.name ??
        asset.collection?.name ??
        asset.collection?.symbol ??
        `ERC-721 #${BigInt(asset.tokenId).toString()}`
      );
    case 'ERC1155':
      return (
        asset.token?.name ??
        asset.collection?.name ??
        `ERC-1155 #${BigInt(asset.tokenId).toString()}`
      );
  }
}

function collectionTitle(collection: DryrunCollection) {
  switch (collection.type) {
    case 'ERC721':
      return (
        collection.collection?.name ??
        collection.collection?.symbol ??
        shortAddress(collection.contractAddress)
      );
    case 'ERC1155':
      return (
        collection.collection?.name ??
        shortAddress(collection.contractAddress)
      );
  }
}

function changeValue(asset: DryrunAsset, amount?: string) {
  if (amount) {
    if (asset.type === 'ERC20' || asset.type === 'NATIVE') {
      const decimals = asset.display?.decimals ?? 0;
      const symbol = asset.type === 'NATIVE'
        ? asset.display?.symbol ?? 'ETH'
        : asset.display?.symbol ?? '';

      return `${compactNumber(formatUnits(BigInt(amount), decimals))}${symbol ? ` ${symbol}` : ''}`;
    }

    return compactNumber(BigInt(amount).toString());
  }

  if (asset.type === 'ERC721' || asset.type === 'ERC1155') {
    return `#${BigInt(asset.tokenId).toString()}`;
  }

  return undefined;
}

function compactNumber(value: string) {
  const [whole, decimal] = value.split('.');
  const groupedWhole = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ',');

  if (!decimal) {
    return groupedWhole;
  }

  return `${groupedWhole}.${decimal.slice(0, 4)}`;
}
