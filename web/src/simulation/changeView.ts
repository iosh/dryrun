import {
  formatHexQuantity,
  formatNativeAmount,
  formatRawAmount,
} from '../lib/formatting.ts';
import { getEnvironment, type EnvironmentId } from './environment.ts';
import type { SimulationChange } from './types.ts';

export type ChangeTone = 'amber' | 'blue' | 'green' | 'red' | 'violet';

export interface ChangeItemViewModel {
  detail?: string;
  identifier?: string;
  label: string;
  title: string;
  tone: ChangeTone;
  value?: string;
}

type AssetChange = Extract<
  SimulationChange,
  { changeType: 'TRANSFER' | 'MINT' | 'BURN' }
>;

export function toChangeItemViewModel(
  change: SimulationChange,
  environmentId: EnvironmentId,
): ChangeItemViewModel {
  switch (change.changeType) {
    case 'NATIVE_TRANSFER':
      return {
        label: 'Transfer',
        title: change.symbol,
        tone: 'blue',
        value: formatFungibleAmount(
          change.rawAmount,
          change.decimals,
          change.symbol,
        ),
      };
    case 'SELF_DESTRUCT_BURN':
      return {
        identifier: change.contractAddress,
        label: 'Self-destruct burn',
        title: change.symbol,
        tone: 'red',
        value: formatFungibleAmount(
          change.rawAmount,
          change.decimals,
          change.symbol,
        ),
      };
    case 'WRAPPED_NATIVE_DEPOSIT':
      return {
        identifier: change.contractAddress,
        label: 'Wrapped native deposit',
        title: tokenName(change, 'Wrapped native'),
        tone: 'violet',
        value: formatFungibleAmount(
          change.rawAmount,
          metadataDecimals(change),
          metadataSymbol(change),
        ),
      };
    case 'WRAPPED_NATIVE_WITHDRAWAL':
      return {
        identifier: change.contractAddress,
        label: 'Wrapped native withdrawal',
        title: tokenName(change, 'Wrapped native'),
        tone: 'amber',
        value: formatFungibleAmount(
          change.rawAmount,
          metadataDecimals(change),
          metadataSymbol(change),
        ),
      };
    case 'ERC20_TRANSFER':
      return {
        identifier: change.contractAddress,
        label: 'ERC-20 transfer',
        title: tokenName(change, 'ERC-20'),
        tone: 'blue',
        value: formatFungibleAmount(
          change.rawAmount,
          metadataDecimals(change),
          metadataSymbol(change),
        ),
      };
    case 'ERC20_APPROVAL':
      return {
        identifier: change.contractAddress,
        label: 'ERC-20 approval',
        title: tokenName(change, 'ERC-20'),
        tone: BigInt(change.approvedAmount) === 0n ? 'amber' : 'green',
        value: formatFungibleAmount(
          change.approvedAmount,
          metadataDecimals(change),
          metadataSymbol(change),
        ),
      };
    case 'ERC721_TRANSFER':
      return {
        identifier: change.contractAddress,
        label: 'ERC-721 transfer',
        title: `${tokenName(change, 'ERC-721')} #${formatHexQuantity(change.tokenId)}`,
        tone: 'blue',
      };
    case 'ERC721_APPROVAL':
      return {
        detail: `Token #${formatHexQuantity(change.tokenId)}`,
        identifier: change.contractAddress,
        label: 'ERC-721 approval',
        title: tokenName(change, 'ERC-721'),
        tone: change.approvedAddress ? 'green' : 'amber',
        value: change.approvedAddress ? 'Approved' : 'Revoked',
      };
    case 'ERC1155_TRANSFER_SINGLE':
      return {
        detail: `Token #${formatHexQuantity(change.tokenId)}`,
        identifier: change.contractAddress,
        label: 'ERC-1155 transfer',
        title: 'ERC-1155',
        tone: 'blue',
        value: formatHexQuantity(change.rawAmount),
      };
    case 'ERC1155_TRANSFER_BATCH':
      return {
        detail: 'Ordered batch transfer',
        identifier: change.contractAddress,
        label: 'ERC-1155 batch',
        title: 'ERC-1155',
        tone: 'blue',
        value: `${change.items.length} ${change.items.length === 1 ? 'item' : 'items'}`,
      };
    case 'TRANSFER':
      return withAsset(change, environmentId, {
        label: 'Transfer',
        tone: 'blue',
      });
    case 'MINT':
      return withAsset(change, environmentId, {
        label: 'Mint',
        tone: 'violet',
      });
    case 'BURN':
      return withAsset(change, environmentId, {
        label: 'Burn',
        tone: 'red',
      });
    case 'ALLOWANCE':
      return {
        identifier: change.contractAddress,
        label: 'Allowance',
        title: tokenName(change, 'ERC-20'),
        tone:
          BigInt(change.rawAmountAfter) >= BigInt(change.rawAmountBefore)
            ? 'green'
            : 'amber',
        value: formatAllowanceDelta(change),
      };
    case 'TOKEN_APPROVAL':
      return {
        detail: `Token #${formatHexQuantity(change.tokenId)}`,
        identifier: change.contractAddress,
        label: 'Token approval',
        title: tokenName(change, 'ERC-721'),
        tone: 'green',
        value: addressTransition(
          change.approvedAddressBefore,
          change.approvedAddressAfter,
        ),
      };
    case 'OPERATOR_APPROVAL':
      if ('approved' in change) {
        return {
          identifier: change.contractAddress,
          label: 'Operator approval',
          title: 'Token collection',
          tone: change.approved ? 'green' : 'amber',
          value: change.approved ? 'Enabled' : 'Disabled',
        };
      }
      return {
        identifier: change.contractAddress,
        label: 'Operator approval',
        title: tokenName(change, change.assetType === 'ERC721' ? 'ERC-721' : 'ERC-1155'),
        tone: 'green',
        value: booleanTransition(change.approvedBefore, change.approvedAfter),
      };
    case 'STAKING_DEPOSIT':
      return coreAmountChange(
        'Staking deposit',
        change.rawAmount,
        'green',
      );
    case 'STAKING_WITHDRAWAL':
      return {
        ...coreAmountChange(
          'Staking withdrawal',
          change.rawAmount,
          'amber',
        ),
        detail: `Reward ${formatNativeAmount(change.rewardRawAmount, 'CFX')}`,
      };
    case 'STAKING_BURN':
      return coreAmountChange(
        'Staking burn',
        change.rawAmount,
        'red',
      );
    case 'STAKING_VOTE_LOCK':
      return {
        detail: `Until block ${formatHexQuantity(change.unlockBlockNumber)}`,
        label: 'Vote lock',
        title: 'Required locked stake',
        tone: 'violet',
        value: `${formatNativeAmount(change.requiredLockedRawAmountBefore, 'CFX')} to ${formatNativeAmount(change.requiredLockedRawAmountAfter, 'CFX')}`,
      };
    case 'POS_REGISTRATION':
      return posChange(
        'PoS registration',
        change.posIdentifier,
        change.newlyLockedVoteCount,
        change.newlyLockedRawAmount,
      );
    case 'POS_STAKE_INCREASE':
      return posChange(
        'PoS stake increase',
        change.posIdentifier,
        change.newlyLockedVoteCount,
        change.newlyLockedRawAmount,
      );
    case 'POS_RETIREMENT_REQUEST':
      return {
        detail: 'Retirement requested',
        identifier: change.posIdentifier,
        label: 'PoS retirement',
        title: 'Votes requested',
        tone: 'amber',
        value: formatHexQuantity(change.requestedVoteCount),
      };
    case 'SPONSORSHIP_DEPOSIT':
      return sponsorshipAmountChange('Sponsor deposit', change);
    case 'SPONSORSHIP_REFUND':
      return sponsorshipAmountChange('Sponsor refund', change);
    case 'SPONSORSHIP_CONFIGURATION':
      return {
        detail: sponsorTransition(change.sponsorBefore, change.sponsorAfter),
        identifier: change.contractAddress,
        label: 'Sponsor configuration',
        title:
          change.sponsoredResource === 'GAS'
            ? 'Gas sponsorship'
            : 'Storage sponsorship',
        tone: 'violet',
        ...('maxSponsoredGasFeeRawAmountAfter' in change
          ? {
              value: `${formatNativeAmount(change.maxSponsoredGasFeeRawAmountBefore, 'CFX')} to ${formatNativeAmount(change.maxSponsoredGasFeeRawAmountAfter, 'CFX')}`,
            }
          : {}),
      };
    case 'SPONSORSHIP_ELIGIBILITY_RULE':
      return {
        detail:
          change.appliesTo.type === 'ALL_ACCOUNTS'
            ? 'Applies to all accounts'
            : 'Applies to one account',
        identifier: change.contractAddress,
        label: 'Sponsor eligibility',
        title: 'Eligibility rule',
        tone: 'violet',
        value: booleanTransition(change.enabledBefore, change.enabledAfter),
      };
    case 'STORAGE_POINT_CONVERSION':
      return {
        detail: 'Converted to storage points',
        identifier: change.contractAddress,
        label: 'Storage points',
        title: 'CFX conversion',
        tone: 'blue',
        value: formatNativeAmount(change.convertedCfxRawAmount, 'CFX'),
      };
    case 'CROSS_SPACE_TRANSFER':
      return {
        label: 'Cross-space transfer',
        title: 'CFX',
        tone: 'blue',
        value: formatNativeAmount(change.rawAmount, 'CFX'),
      };
  }
}

function withAsset(
  change: AssetChange,
  environmentId: EnvironmentId,
  base: Pick<ChangeItemViewModel, 'label' | 'tone'>,
): ChangeItemViewModel {
  return {
    ...base,
    identifier:
      change.assetType === 'NATIVE' ? undefined : change.contractAddress,
    title: assetTitle(change, environmentId),
    value: assetValue(change, environmentId),
  };
}

function assetTitle(change: AssetChange, environmentId: EnvironmentId) {
  switch (change.assetType) {
    case 'NATIVE':
      return metadataSymbol(change) ?? getEnvironment(environmentId).nativeSymbol;
    case 'ERC20':
      return tokenName(change, 'ERC-20');
    case 'ERC721':
      return `${tokenName(change, 'ERC-721')} #${formatHexQuantity(change.tokenId)}`;
    case 'ERC1155':
      return `ERC-1155 #${formatHexQuantity(change.tokenId)}`;
  }
}

function assetValue(change: AssetChange, environmentId: EnvironmentId) {
  switch (change.assetType) {
    case 'NATIVE':
      return formatNativeAmount(
        change.rawAmount,
        metadataSymbol(change) ?? getEnvironment(environmentId).nativeSymbol,
      );
    case 'ERC20': {
      const symbol = metadataSymbol(change);
      const amount = formatTokenAmount(
        change.rawAmount,
        metadataDecimals(change),
      );
      return symbol ? `${amount} ${symbol}` : amount;
    }
    case 'ERC721':
      return undefined;
    case 'ERC1155':
      return formatHexQuantity(change.rawAmount);
  }
}

function tokenName(
  change: object,
  fallback: string,
) {
  return metadataSymbol(change) ?? metadataName(change) ?? fallback;
}

function metadataName(value: object) {
  return 'name' in value && typeof value.name === 'string'
    ? value.name
    : undefined;
}

function metadataSymbol(value: object) {
  return 'symbol' in value && typeof value.symbol === 'string'
    ? value.symbol
    : undefined;
}

function metadataDecimals(value: object) {
  return 'decimals' in value && typeof value.decimals === 'number'
    ? value.decimals
    : 0;
}

function formatTokenAmount(rawAmount: string, decimals = 0) {
  return formatRawAmount(rawAmount, decimals);
}

function formatFungibleAmount(
  rawAmount: string,
  decimals: number,
  symbol?: string,
) {
  const amount = formatTokenAmount(rawAmount, decimals);
  return symbol ? `${amount} ${symbol}` : amount;
}

function formatAllowanceDelta(
  change: Extract<SimulationChange, { changeType: 'ALLOWANCE' }>,
) {
  const delta =
    BigInt(change.rawAmountAfter) - BigInt(change.rawAmountBefore);
  if (delta === 0n) return 'No change';

  const amount = formatTokenAmount(
    (delta < 0n ? -delta : delta).toString(),
    metadataDecimals(change),
  );
  const symbol = metadataSymbol(change);
  return `${delta > 0n ? '+' : '-'}${amount}${symbol ? ` ${symbol}` : ''}`;
}

function addressTransition(before: string | null, after: string | null) {
  if (before === after) return after ? 'Unchanged' : 'None';
  if (!after) return 'Revoked';
  return before ? 'Changed' : 'Approved';
}

function sponsorTransition(before: string | null, after: string | null) {
  if (before === after) return after ? 'Sponsor unchanged' : 'No sponsor';
  if (!after) return 'Sponsor removed';
  return before ? 'Sponsor changed' : 'Sponsor added';
}

function booleanTransition(before: boolean, after: boolean) {
  if (before === after) return after ? 'Enabled' : 'Disabled';
  return `${before ? 'Enabled' : 'Disabled'} to ${after ? 'Enabled' : 'Disabled'}`;
}

function coreAmountChange(
  label: string,
  rawAmount: string,
  tone: ChangeTone,
): ChangeItemViewModel {
  return {
    label,
    title: 'CFX',
    tone,
    value: formatNativeAmount(rawAmount, 'CFX'),
  };
}

function posChange(
  label: string,
  identifier: string,
  voteCount: string,
  rawAmount: string,
): ChangeItemViewModel {
  return {
    detail: `Locked ${formatNativeAmount(rawAmount, 'CFX')}`,
    identifier,
    label,
    title: 'Newly locked votes',
    tone: 'violet',
    value: formatHexQuantity(voteCount),
  };
}

function sponsorshipAmountChange(
  label: string,
  change: Extract<
    SimulationChange,
    { changeType: 'SPONSORSHIP_DEPOSIT' | 'SPONSORSHIP_REFUND' }
  >,
): ChangeItemViewModel {
  return {
    detail:
      change.sponsoredResource === 'GAS'
        ? 'Gas sponsorship'
        : 'Storage sponsorship',
    identifier: change.contractAddress,
    label,
    title: 'CFX',
    tone: change.changeType === 'SPONSORSHIP_DEPOSIT' ? 'green' : 'amber',
    value: formatNativeAmount(change.rawAmount, 'CFX'),
  };
}
