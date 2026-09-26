import {
  formatAmount,
  formatHexQuantity,
  formatNativeAmount,
  shortHex,
} from '../lib/formatting.ts';
import type { AssetChange } from './rpc.ts';
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
  switch (change.type) {
    case 'nativeTransfer':
      return {
        label: 'Transfer',
        title: change.symbol,
        tone: 'blue',
        value: formatAmount(
          change.rawAmount,
          change.decimals,
          change.symbol,
        ),
      };
    case 'selfDestructBurn':
      return {
        identifier: change.contractAddress,
        label: 'Self-destruct burn',
        title: change.symbol,
        tone: 'red',
        value: formatAmount(
          change.rawAmount,
          change.decimals,
          change.symbol,
        ),
      };
    case 'accountDelegation':
      return {
        detail: `${change.before.delegate ? shortHex(change.before.delegate) : 'No delegate'} -> ${change.after.delegate ? shortHex(change.after.delegate) : 'No delegate'}`,
        identifier: change.account,
        label: 'Account delegation',
        title: 'EIP-7702',
        tone: 'violet',
      };
    case 'wrappedNativeDeposit':
      return {
        identifier: change.contractAddress,
        label: 'Wrapped native deposit',
        title: tokenName(change, 'Wrapped native'),
        tone: 'violet',
        value: formatAmount(
          change.rawAmount,
          change.decimals,
          metadataSymbol(change),
        ),
      };
    case 'wrappedNativeWithdrawal':
      return {
        identifier: change.contractAddress,
        label: 'Wrapped native withdrawal',
        title: tokenName(change, 'Wrapped native'),
        tone: 'amber',
        value: formatAmount(
          change.rawAmount,
          change.decimals,
          metadataSymbol(change),
        ),
      };
    case 'erc20Transfer':
      return erc20AmountView(change, 'ERC-20 transfer', 'blue');
    case 'erc20Mint':
      return erc20AmountView(change, 'ERC-20 mint', 'green');
    case 'erc20Burn':
      return erc20AmountView(change, 'ERC-20 burn', 'red');
    case 'erc20Approval':
      return {
        detail: `${formatAmount(change.before, change.decimals, metadataSymbol(change))} -> ${formatAmount(change.after, change.decimals, metadataSymbol(change))}`,
        identifier: change.contractAddress,
        label: 'ERC-20 approval',
        title: tokenName(change, 'ERC-20'),
        tone: BigInt(change.after) === 0n ? 'amber' : 'green',
      };
    case 'erc721Transfer':
      return erc721View(change, 'ERC-721 transfer', 'blue');
    case 'erc721Mint':
      return erc721View(change, 'ERC-721 mint', 'green');
    case 'erc721Burn':
      return erc721View(change, 'ERC-721 burn', 'red');
    case 'erc721Approval':
      return {
        detail: `Token #${formatHexQuantity(change.tokenId)} | ${approvalAddress(change.before)} -> ${approvalAddress(change.after)}`,
        identifier: change.contractAddress,
        label: 'ERC-721 approval',
        title: tokenName(change, 'ERC-721'),
        tone: change.after ? 'green' : 'amber',
      };
    case 'operatorApproval':
      return {
        detail: `${change.before ? 'Enabled' : 'Disabled'} -> ${change.after ? 'Enabled' : 'Disabled'}`,
        identifier: change.contractAddress,
        label: 'Operator approval',
        title: 'Token collection',
        tone: change.after ? 'green' : 'amber',
      };
    case 'erc1155TransferSingle':
      return erc1155SingleView(change, 'ERC-1155 transfer', 'blue');
    case 'erc1155MintSingle':
      return erc1155SingleView(change, 'ERC-1155 mint', 'green');
    case 'erc1155BurnSingle':
      return erc1155SingleView(change, 'ERC-1155 burn', 'red');
    case 'erc1155TransferBatch':
      return erc1155BatchView(change, 'ERC-1155 batch', 'blue');
    case 'erc1155MintBatch':
      return erc1155BatchView(change, 'ERC-1155 batch mint', 'green');
    case 'erc1155BurnBatch':
      return erc1155BatchView(change, 'ERC-1155 batch burn', 'red');
    case 'stakingDeposit':
      return coreAmountChange(
        'Staking deposit',
        change.rawAmount,
        'green',
      );
    case 'stakingWithdrawal':
      return {
        ...coreAmountChange(
          'Staking withdrawal',
          change.principalRawAmount,
          'amber',
        ),
        detail: `Reward ${formatNativeAmount(change.rewardRawAmount, 'CFX')}`,
      };
    case 'stakingVoteLock':
      return {
        detail: `Until block ${formatHexQuantity(change.unlockBlockNumber)}`,
        label: 'Vote lock',
        title: 'Required locked stake',
        tone: 'violet',
        value: formatNativeAmount(change.requiredLockedRawAmount, 'CFX'),
      };
    case 'posRegistration':
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
    case 'posStakeIncrease':
      return posChange(
        'PoS stake increase',
        'Added votes',
        change.identifier,
        change.addedVoteCount,
        change.addedLockedRawAmount,
      );
    case 'posRetirementRequest':
      return {
        detail: 'Retirement requested',
        identifier: change.identifier,
        label: 'PoS retirement',
        title: 'Votes requested',
        tone: 'amber',
        value: formatHexQuantity(change.requestedVoteCount),
      };
    case 'governanceVoteCast': {
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
    case 'sponsorshipFunding':
      return {
        detail: sponsorshipFundingDetail(change),
        identifier: change.contractAddress,
        label: 'Sponsor funding',
        title:
          change.resource === 'gas'
            ? 'Gas sponsorship'
            : 'Storage sponsorship',
        tone: 'violet',
        value: formatNativeAmount(change.contributedRawAmount, 'CFX'),
      };
    case 'contractAdminSet':
      return {
        detail: change.admin === null ? 'Admin cleared' : 'Admin set',
        identifier: change.contractAddress,
        label: 'Contract admin',
        title: 'Admin control',
        tone: 'violet',
        ...(change.admin === null ? {} : { value: shortHex(change.admin) }),
      };
    case 'sponsorshipAccessRule':
    case 'sponsorshipAccessRuleSet':
      return {
        detail:
          change.scope.type === 'allAccounts'
            ? 'All accounts'
            : 'One account',
        identifier: change.contractAddress,
        label: 'Sponsor eligibility',
        title: change.enabled ? 'Enabled' : 'Disabled',
        tone: 'violet',
      };
    case 'storagePointConversion':
      return {
        detail: `Pool ${formatNativeAmount(change.fromSponsorPoolRawAmount, 'CFX')} | Collateral ${formatNativeAmount(change.fromStorageCollateralRawAmount, 'CFX')}`,
        identifier: change.contractAddress,
        label: 'Storage points',
        title: 'CFX conversion',
        tone: 'blue',
        value: formatNativeAmount(
          (
            BigInt(change.fromSponsorPoolRawAmount) +
            BigInt(change.fromStorageCollateralRawAmount)
          ).toString(),
          'CFX',
        ),
      };
    case 'crossSpaceNativeTransfer':
      return {
        label: 'Cross-space transfer',
        title: 'CFX',
        tone: 'blue',
        value: formatNativeAmount(change.rawAmount, 'CFX'),
      };
    case 'gasSponsorship':
      return {
        detail: `Cap ${formatNativeAmount(change.gasFeeUpperBoundRawAmount, 'CFX')}`,
        identifier: change.contractAddress,
        label: 'Gas sponsorship',
        title: 'Final pool balance',
        tone: 'violet',
        value: formatNativeAmount(change.balanceRawAmount, 'CFX'),
      };
    case 'storageSponsorship':
      return {
        detail: change.storagePoints
          ? `Storage points: ${formatHexQuantity(change.storagePoints.unused)} unused, ${formatHexQuantity(change.storagePoints.used)} used`
          : undefined,
        identifier: change.contractAddress,
        label: 'Storage sponsorship',
        title: 'Final pool balance',
        tone: 'violet',
        value: formatNativeAmount(change.balanceRawAmount, 'CFX'),
      };
    case 'storageCollateral':
      return {
        ...coreAmountChange('Storage collateral', change.rawAmount, 'blue'),
        identifier: change.contractAddress,
        title: 'Final collateral balance',
      };
    case 'contractAdmin':
      return {
        identifier: change.contractAddress,
        label: 'Contract admin',
        title: change.state === null ? 'Contract removed' : 'Final admin',
        tone: 'violet',
        value: change.state?.admin ? shortHex(change.state.admin) : 'None',
      };
  }
}

function erc20AmountView(
  change: Extract<AssetChange, { type: 'erc20Transfer' | 'erc20Mint' | 'erc20Burn' }>,
  label: string,
  tone: ChangeTone,
): ChangeItemViewModel {
  return {
    identifier: change.contractAddress,
    label,
    title: tokenName(change, 'ERC-20'),
    tone,
    value: formatAmount(
      change.rawAmount,
      change.decimals,
      metadataSymbol(change),
    ),
  };
}

function erc721View(
  change: Extract<AssetChange, { type: 'erc721Transfer' | 'erc721Mint' | 'erc721Burn' }>,
  label: string,
  tone: ChangeTone,
): ChangeItemViewModel {
  return {
    identifier: change.contractAddress,
    label,
    title: `${tokenName(change, 'ERC-721')} #${formatHexQuantity(change.tokenId)}`,
    tone,
  };
}

function erc1155SingleView(
  change: Extract<
    AssetChange,
    { type: 'erc1155TransferSingle' | 'erc1155MintSingle' | 'erc1155BurnSingle' }
  >,
  label: string,
  tone: ChangeTone,
): ChangeItemViewModel {
  return {
    detail: `Token #${formatHexQuantity(change.tokenId)}`,
    identifier: change.contractAddress,
    label,
    title: 'ERC-1155',
    tone,
    value: formatHexQuantity(change.rawAmount),
  };
}

function erc1155BatchView(
  change: Extract<
    AssetChange,
    { type: 'erc1155TransferBatch' | 'erc1155MintBatch' | 'erc1155BurnBatch' }
  >,
  label: string,
  tone: ChangeTone,
): ChangeItemViewModel {
  return {
    detail: 'Ordered batch',
    identifier: change.contractAddress,
    label,
    title: 'ERC-1155',
    tone,
    value: `${change.items.length} ${change.items.length === 1 ? 'item' : 'items'}`,
  };
}

function approvalAddress(address: string | null) {
  return address ? shortHex(address) : 'None';
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

function sponsorshipFundingDetail(
  change: Extract<SimulationChange, { type: 'sponsorshipFunding' }>,
) {
  const details = [
    change.replacement === null
      ? 'Sponsor pool funded'
      : `Replaced ${shortHex(change.replacement.previousSponsor)} | Refunded ${formatNativeAmount(change.replacement.poolRefundedRawAmount, 'CFX')}`,
    `Pool ${formatNativeAmount(change.poolCreditedRawAmount, 'CFX')}`,
  ];
  if (change.resource === 'gas') {
    details.push(
      `Cap ${formatNativeAmount(change.gasFeeUpperBound, 'CFX')}`,
    );
  } else if (change.replacement !== null) {
    details.push(
      `Collateral compensation ${formatNativeAmount(change.replacement.collateralCompensationRawAmount, 'CFX')}`,
    );
  }
  return details.join(' | ');
}
