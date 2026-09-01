import {
  formatHexQuantity,
  formatNativeAmount,
  formatRawAmount,
  shortHex,
} from '../lib/formatting.ts';
import type { EvmChange } from './rpc.ts';
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
  if ('type' in change) {
    return toEvmChangeItemViewModel(change);
  }

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
    case 'SPONSORSHIP_FUNDING':
      return {
        detail: sponsorshipFundingDetail(change),
        identifier: change.contractAddress,
        label: 'Sponsor funding',
        title:
          change.sponsoredResource === 'GAS'
            ? 'Gas sponsorship'
            : 'Storage sponsorship',
        tone: 'violet',
        value: formatNativeAmount(change.contributedRawAmount, 'CFX'),
      };
    case 'CONTRACT_ADMIN_SET':
      return {
        detail: change.admin === null ? 'Admin cleared' : 'Admin set',
        identifier: change.contractAddress,
        label: 'Contract admin',
        title: 'Admin control',
        tone: 'violet',
        ...(change.admin === null ? {} : { value: shortHex(change.admin) }),
      };
    case 'SPONSORSHIP_ACCESS_RULE_SET':
      return {
        detail:
          change.scope.type === 'ALL_ACCOUNTS'
            ? 'All accounts'
            : 'One account',
        identifier: change.contractAddress,
        label: 'Sponsor eligibility',
        title: change.enabled ? 'Enabled' : 'Disabled',
        tone: 'violet',
      };
    case 'STORAGE_POINT_CONVERSION':
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
    case 'CROSS_SPACE_NATIVE_TRANSFER':
      return {
        label: 'Cross-space transfer',
        title: 'CFX',
        tone: 'blue',
        value: formatNativeAmount(change.rawAmount, 'CFX'),
      };
    case 'ESPACE': {
      const nested = toChangeItemViewModel(change.change);
      return {
        ...nested,
        label: `eSpace ${nested.label}`,
      };
    }
  }
}

function toEvmChangeItemViewModel(change: EvmChange): ChangeItemViewModel {
  switch (change.type) {
    case 'nativeTransfer':
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
    case 'selfDestructBurn':
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
        value: formatFungibleAmount(
          change.rawAmount,
          metadataDecimals(change),
          metadataSymbol(change),
        ),
      };
    case 'wrappedNativeWithdrawal':
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
    case 'erc20Transfer':
      return evmErc20AmountView(change, 'ERC-20 transfer', 'blue');
    case 'erc20Mint':
      return evmErc20AmountView(change, 'ERC-20 mint', 'green');
    case 'erc20Burn':
      return evmErc20AmountView(change, 'ERC-20 burn', 'red');
    case 'erc20Approval':
      return {
        detail: `${formatFungibleAmount(change.before, metadataDecimals(change), metadataSymbol(change))} -> ${formatFungibleAmount(change.after, metadataDecimals(change), metadataSymbol(change))}`,
        identifier: change.contractAddress,
        label: 'ERC-20 approval',
        title: tokenName(change, 'ERC-20'),
        tone: BigInt(change.after) === 0n ? 'amber' : 'green',
      };
    case 'erc721Transfer':
      return evmErc721View(change, 'ERC-721 transfer', 'blue');
    case 'erc721Mint':
      return evmErc721View(change, 'ERC-721 mint', 'green');
    case 'erc721Burn':
      return evmErc721View(change, 'ERC-721 burn', 'red');
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
      return evmErc1155SingleView(change, 'ERC-1155 transfer', 'blue');
    case 'erc1155MintSingle':
      return evmErc1155SingleView(change, 'ERC-1155 mint', 'green');
    case 'erc1155BurnSingle':
      return evmErc1155SingleView(change, 'ERC-1155 burn', 'red');
    case 'erc1155TransferBatch':
      return evmErc1155BatchView(change, 'ERC-1155 batch', 'blue');
    case 'erc1155MintBatch':
      return evmErc1155BatchView(change, 'ERC-1155 batch mint', 'green');
    case 'erc1155BurnBatch':
      return evmErc1155BatchView(change, 'ERC-1155 batch burn', 'red');
  }
}

function evmErc20AmountView(
  change: Extract<EvmChange, { type: 'erc20Transfer' | 'erc20Mint' | 'erc20Burn' }>,
  label: string,
  tone: ChangeTone,
): ChangeItemViewModel {
  return {
    identifier: change.contractAddress,
    label,
    title: tokenName(change, 'ERC-20'),
    tone,
    value: formatFungibleAmount(
      change.rawAmount,
      metadataDecimals(change),
      metadataSymbol(change),
    ),
  };
}

function evmErc721View(
  change: Extract<EvmChange, { type: 'erc721Transfer' | 'erc721Mint' | 'erc721Burn' }>,
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

function evmErc1155SingleView(
  change: Extract<
    EvmChange,
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

function evmErc1155BatchView(
  change: Extract<
    EvmChange,
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
  change: Extract<SimulationChange, { changeType: 'SPONSORSHIP_FUNDING' }>,
) {
  const details = [
    change.replacement === null
      ? 'Sponsor pool funded'
      : `Replaced ${shortHex(change.replacement.previousSponsor)} | Refunded ${formatNativeAmount(change.replacement.poolRefundedRawAmount, 'CFX')}`,
    `Pool ${formatNativeAmount(change.poolCreditedRawAmount, 'CFX')}`,
  ];
  if (change.sponsoredResource === 'GAS') {
    details.push(
      `Cap ${formatNativeAmount(change.gasFeeUpperBoundRawAmount, 'CFX')}`,
    );
  } else if (change.replacement !== null) {
    details.push(
      `Collateral compensation ${formatNativeAmount(change.replacement.collateralCompensationRawAmount, 'CFX')}`,
    );
  }
  return details.join(' | ');
}
