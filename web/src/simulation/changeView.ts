import {
  formatHexQuantity,
  formatNativeAmount,
  formatRawAmount,
  shortHex,
} from '../lib/formatting.ts';
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

export function toChangeItemViewModel(
  change: SimulationChange,
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
    case 'NATIVE_BURN':
      return {
        label: 'Burn',
        title: change.symbol,
        tone: 'red',
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
    case 'OPERATOR_APPROVAL':
      return {
        identifier: change.contractAddress,
        label: 'Operator approval',
        title: 'Token collection',
        tone: change.approved ? 'green' : 'amber',
        value: change.approved ? 'Enabled' : 'Disabled',
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
          change.principalRawAmount,
          'amber',
        ),
        detail: `Reward ${formatNativeAmount(change.rewardRawAmount, 'CFX')}`,
      };
    case 'STAKING_VOTE_LOCK':
      return {
        detail: `Until block ${formatHexQuantity(change.unlockBlockNumber)}`,
        label: 'Vote lock',
        title: 'Required locked stake',
        tone: 'violet',
        value: formatNativeAmount(change.requiredLockedRawAmount, 'CFX'),
      };
    case 'POS_REGISTRATION':
      return {
        ...posChange(
          'PoS registration',
          'Initial votes',
          change.identifier,
          change.initialVoteCount,
          change.lockedRawAmount,
        ),
        detail: `Locked ${formatNativeAmount(change.lockedRawAmount, 'CFX')} | BLS ${shortHex(change.blsPublicKey, 12, 8)} | VRF ${shortHex(change.vrfPublicKey, 12, 8)}`,
      };
    case 'POS_STAKE_INCREASE':
      return posChange(
        'PoS stake increase',
        'Added votes',
        change.identifier,
        change.addedVoteCount,
        change.addedLockedRawAmount,
      );
    case 'POS_RETIREMENT_REQUEST':
      return {
        detail: 'Retirement requested',
        identifier: change.identifier,
        label: 'PoS retirement',
        title: 'Votes requested',
        tone: 'amber',
        value: formatHexQuantity(change.requestedVoteCount),
      };
    case 'GOVERNANCE_VOTE_CAST': {
      const replacements = change.votes.filter(
        (vote) => vote.replacedAllocation !== null,
      ).length;
      return {
        detail:
          replacements === 0
            ? `${change.votes.length} parameter votes`
            : `${replacements} of ${change.votes.length} replaced`,
        label: 'Governance vote',
        title: `Round ${formatHexQuantity(change.round)}`,
        tone: 'violet',
      };
    }
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
  title: string,
  identifier: string,
  voteCount: string,
  rawAmount: string,
): ChangeItemViewModel {
  return {
    detail: `Locked ${formatNativeAmount(rawAmount, 'CFX')}`,
    identifier,
    label,
    title,
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
