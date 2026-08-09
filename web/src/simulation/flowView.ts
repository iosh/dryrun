import {
  toChangeItemViewModel,
  type ChangeTone,
} from './changeView.ts';
import { formatHexQuantity } from '../lib/formatting.ts';
import { getEnvironment, type EnvironmentId } from './environment.ts';
import type { SimulationChange } from './types.ts';

export interface ChangeAddressViewModel {
  address: string;
  label: string;
}

export type FlowEndpoint =
  | {
      address: string;
      context?: string;
      kind: 'address';
      label: string;
    }
  | {
      kind: 'terminal';
      label: 'Burn' | 'Mint';
    };

export interface AssetFlowItemViewModel {
  assetKey: string;
  assetIdentifier?: string;
  assetTitle: string;
  decimals: number;
  from: FlowEndpoint;
  label: string;
  rawAmount: string;
  to: FlowEndpoint;
  tone: ChangeTone;
  value: string;
}

type ConfluxAssetMovementChange = Extract<
  SimulationChange,
  { changeType: 'BURN' | 'MINT' | 'TRANSFER' }
>;

export function toAssetFlowItemViewModels(
  change: SimulationChange,
  environmentId: EnvironmentId,
): AssetFlowItemViewModel[] {
  const view = toChangeItemViewModel(change, environmentId);

  switch (change.changeType) {
    case 'NATIVE_TRANSFER':
      return [
        {
          assetKey: `NATIVE:${change.symbol}`,
          assetTitle: change.symbol,
          decimals: change.decimals,
          from: addressEndpoint('From', change.from),
          label: view.label,
          rawAmount: change.rawAmount,
          to: addressEndpoint('To', change.to),
          tone: view.tone,
          value: view.value ?? view.title,
        },
      ];
    case 'SELF_DESTRUCT_BURN':
      return [
        {
          assetKey: `NATIVE:${change.symbol}`,
          assetTitle: change.symbol,
          decimals: change.decimals,
          from: addressEndpoint('Contract', change.contractAddress),
          label: view.label,
          rawAmount: change.rawAmount,
          to: { kind: 'terminal', label: 'Burn' },
          tone: view.tone,
          value: view.value ?? view.title,
        },
      ];
    case 'ERC20_TRANSFER':
      return [
        {
          assetKey: `ERC20:${normalizeAddress(change.contractAddress)}`,
          assetIdentifier: change.contractAddress,
          assetTitle: view.title,
          decimals: change.decimals ?? 0,
          from: addressEndpoint('From', change.from),
          label: view.label,
          rawAmount: change.rawAmount,
          to: addressEndpoint('To', change.to),
          tone: view.tone,
          value: view.value ?? view.title,
        },
      ];
    case 'ERC721_TRANSFER':
      return [
        {
          assetKey: `ERC721:${normalizeAddress(change.contractAddress)}:${change.tokenId}`,
          assetIdentifier: change.contractAddress,
          assetTitle: view.title,
          decimals: 0,
          from: addressEndpoint('From', change.from),
          label: view.label,
          rawAmount: '0x1',
          to: addressEndpoint('To', change.to),
          tone: view.tone,
          value: view.title,
        },
      ];
    case 'ERC1155_TRANSFER_SINGLE': {
      const assetTitle = `ERC-1155 #${formatHexQuantity(change.tokenId)}`;
      return [
        {
          assetKey: `ERC1155:${normalizeAddress(change.contractAddress)}:${change.tokenId}`,
          assetIdentifier: change.contractAddress,
          assetTitle,
          decimals: 0,
          from: addressEndpoint('From', change.from),
          label: view.label,
          rawAmount: change.rawAmount,
          to: addressEndpoint('To', change.to),
          tone: view.tone,
          value: `${formatHexQuantity(change.rawAmount)} ${assetTitle}`,
        },
      ];
    }
    case 'ERC1155_TRANSFER_BATCH':
      return change.items.map((item) => {
        const assetTitle = `ERC-1155 #${formatHexQuantity(item.tokenId)}`;
        return {
          assetKey: `ERC1155:${normalizeAddress(change.contractAddress)}:${item.tokenId}`,
          assetIdentifier: change.contractAddress,
          assetTitle,
          decimals: 0,
          from: addressEndpoint('From', change.from),
          label: view.label,
          rawAmount: item.rawAmount,
          to: addressEndpoint('To', change.to),
          tone: view.tone,
          value: `${formatHexQuantity(item.rawAmount)} ${assetTitle}`,
        };
      });
    case 'TRANSFER':
      return [
        {
          assetKey: movementAssetKey(change, environmentId),
          assetIdentifier: view.identifier,
          assetTitle: view.title,
          ...movementAmount(change),
          from: addressEndpoint('From', change.from),
          label: view.label,
          to: addressEndpoint('To', change.to),
          tone: view.tone,
          value: movementValue(change, view.value, view.title),
        },
      ];
    case 'MINT':
      return [
        {
          assetKey: movementAssetKey(change, environmentId),
          assetIdentifier: view.identifier,
          assetTitle: view.title,
          ...movementAmount(change),
          from: { kind: 'terminal', label: 'Mint' },
          label: view.label,
          to: addressEndpoint('To', change.to),
          tone: view.tone,
          value: movementValue(change, view.value, view.title),
        },
      ];
    case 'BURN':
      return [
        {
          assetKey: movementAssetKey(change, environmentId),
          assetIdentifier: view.identifier,
          assetTitle: view.title,
          ...movementAmount(change),
          from: addressEndpoint('From', change.from),
          label: view.label,
          to: { kind: 'terminal', label: 'Burn' },
          tone: view.tone,
          value: movementValue(change, view.value, view.title),
        },
      ];
    case 'CROSS_SPACE_TRANSFER':
      return [
        {
          assetKey: 'NATIVE:CFX',
          assetTitle: view.title,
          decimals: 18,
          from: addressEndpoint(
            'From',
            change.from.address,
            spaceLabel(change.from.space),
          ),
          label: view.label,
          rawAmount: change.rawAmount,
          to: addressEndpoint(
            'To',
            change.to.address,
            spaceLabel(change.to.space),
          ),
          tone: view.tone,
          value: view.value ?? view.title,
        },
      ];
    default:
      return [];
  }
}

export function getChangeAddresses(
  change: SimulationChange,
): ChangeAddressViewModel[] {
  switch (change.changeType) {
    case 'NATIVE_TRANSFER':
      return [
        { address: change.from, label: 'From' },
        { address: change.to, label: 'To' },
      ];
    case 'SELF_DESTRUCT_BURN':
      return [{ address: change.contractAddress, label: 'Contract' }];
    case 'WRAPPED_NATIVE_DEPOSIT':
    case 'WRAPPED_NATIVE_WITHDRAWAL':
      return [
        { address: change.account, label: 'Account' },
        { address: change.contractAddress, label: 'Wrapper contract' },
      ];
    case 'ERC20_TRANSFER':
    case 'ERC721_TRANSFER':
      return [
        { address: change.from, label: 'From' },
        { address: change.to, label: 'To' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'ERC1155_TRANSFER_SINGLE':
    case 'ERC1155_TRANSFER_BATCH':
      return [
        { address: change.from, label: 'From' },
        { address: change.to, label: 'To' },
        { address: change.operator, label: 'Operator' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'ERC20_APPROVAL':
      return [
        { address: change.owner, label: 'Owner' },
        { address: change.spender, label: 'Spender' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'ERC721_APPROVAL':
      return compactAddresses([
        { address: change.owner, label: 'Owner' },
        change.approvedAddress
          ? { address: change.approvedAddress, label: 'Approved address' }
          : null,
        { address: change.contractAddress, label: 'Asset contract' },
      ]);
    case 'TRANSFER':
      return withContractAddress(change, [
        { address: change.from, label: 'From' },
        { address: change.to, label: 'To' },
      ]);
    case 'MINT':
      return withContractAddress(change, [
        { address: change.to, label: 'To' },
      ]);
    case 'BURN':
      return withContractAddress(change, [
        { address: change.from, label: 'From' },
      ]);
    case 'ALLOWANCE':
      return [
        { address: change.owner, label: 'Owner' },
        { address: change.spender, label: 'Spender' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'TOKEN_APPROVAL':
      return compactAddresses([
        change.approvedAddressBefore
          ? { address: change.approvedAddressBefore, label: 'Approved before' }
          : null,
        change.approvedAddressAfter
          ? { address: change.approvedAddressAfter, label: 'Approved after' }
          : null,
        { address: change.contractAddress, label: 'Asset contract' },
      ]);
    case 'OPERATOR_APPROVAL':
      return [
        { address: change.owner, label: 'Owner' },
        { address: change.operator, label: 'Operator' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'STAKING_DEPOSIT':
    case 'STAKING_WITHDRAWAL':
    case 'STAKING_BURN':
    case 'STAKING_VOTE_LOCK':
    case 'POS_REGISTRATION':
    case 'POS_STAKE_INCREASE':
    case 'POS_RETIREMENT_REQUEST':
      return [{ address: change.account, label: 'Account' }];
    case 'SPONSORSHIP_DEPOSIT':
    case 'SPONSORSHIP_REFUND':
      return [
        { address: change.sponsor, label: 'Sponsor' },
        { address: change.contractAddress, label: 'Contract' },
      ];
    case 'SPONSORSHIP_CONFIGURATION':
      return compactAddresses([
        change.sponsorBefore
          ? { address: change.sponsorBefore, label: 'Sponsor before' }
          : null,
        change.sponsorAfter
          ? { address: change.sponsorAfter, label: 'Sponsor after' }
          : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'SPONSORSHIP_ELIGIBILITY_RULE':
      return compactAddresses([
        change.appliesTo.type === 'ACCOUNT'
          ? { address: change.appliesTo.address, label: 'Account' }
          : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'STORAGE_POINT_CONVERSION':
      return [{ address: change.contractAddress, label: 'Contract' }];
    case 'CROSS_SPACE_TRANSFER':
      return [
        {
          address: change.from.address,
          label: `From / ${spaceLabel(change.from.space)}`,
        },
        {
          address: change.to.address,
          label: `To / ${spaceLabel(change.to.space)}`,
        },
      ];
  }
}

export function normalizeAddress(address: string) {
  return address.toLowerCase();
}

function addressEndpoint(
  label: string,
  address: string,
  context?: string,
): FlowEndpoint {
  return { address, context, kind: 'address', label };
}

function movementAssetKey(
  change: ConfluxAssetMovementChange,
  environmentId: EnvironmentId,
) {
  switch (change.assetType) {
    case 'NATIVE':
      return `NATIVE:${'symbol' in change && typeof change.symbol === 'string' ? change.symbol : getEnvironment(environmentId).nativeSymbol}`;
    case 'ERC20':
      return `ERC20:${normalizeAddress(change.contractAddress)}`;
    case 'ERC721':
      return `ERC721:${normalizeAddress(change.contractAddress)}:${change.tokenId}`;
    case 'ERC1155':
      return `ERC1155:${normalizeAddress(change.contractAddress)}:${change.tokenId}`;
  }
}

function movementAmount(change: ConfluxAssetMovementChange) {
  if (change.assetType === 'ERC721') {
    return { decimals: 0, rawAmount: '0x1' };
  }

  return {
    decimals:
      change.assetType === 'NATIVE'
        ? 18
        : 'decimals' in change && typeof change.decimals === 'number'
          ? change.decimals
          : 0,
    rawAmount: change.rawAmount,
  };
}

function movementValue(
  change: ConfluxAssetMovementChange,
  value: string | undefined,
  title: string,
) {
  if (change.assetType === 'ERC721') return title;
  if (change.assetType === 'ERC1155') {
    return value ? `${value} ${title}` : title;
  }
  return value ?? title;
}

function withContractAddress(
  change: SimulationChange,
  addresses: ChangeAddressViewModel[],
) {
  if ('contractAddress' in change) {
    addresses.push({
      address: change.contractAddress,
      label: 'Asset contract',
    });
  }
  return addresses;
}

function compactAddresses(
  addresses: Array<ChangeAddressViewModel | null>,
) {
  return addresses.filter(
    (address): address is ChangeAddressViewModel => address !== null,
  );
}

function spaceLabel(space: 'CORE_SPACE' | 'ESPACE') {
  return space === 'CORE_SPACE' ? 'Core' : 'eSpace';
}
