import {
  toChangeItemViewModel,
  type ChangeTone,
} from './changeView.ts';
import { formatHexQuantity } from '../lib/formatting.ts';
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

export function toAssetFlowItemViewModels(
  change: SimulationChange,
): AssetFlowItemViewModel[] {
  const view = toChangeItemViewModel(change);

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
    case 'NATIVE_BURN':
      return [
        {
          assetKey: `NATIVE:${change.symbol}`,
          assetTitle: change.symbol,
          decimals: change.decimals,
          from: addressEndpoint('From', change.from),
          label: view.label,
          rawAmount: change.rawAmount,
          to: { kind: 'terminal', label: 'Burn' },
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
    case 'CROSS_SPACE_NATIVE_TRANSFER':
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
    case 'ESPACE':
      return toAssetFlowItemViewModels(change.change).map((flow) => ({
        ...flow,
        from: withAddressContext(flow.from, 'eSpace'),
        label: view.label,
        to: withAddressContext(flow.to, 'eSpace'),
      }));
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
    case 'NATIVE_BURN':
      return [{ address: change.from, label: 'From' }];
    case 'SELF_DESTRUCT_BURN':
      return [{ address: change.contractAddress, label: 'Contract' }];
    case 'ACCOUNT_DELEGATION':
      return [{ address: change.account, label: 'Account' }];
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
    case 'OPERATOR_APPROVAL':
      return [
        { address: change.owner, label: 'Owner' },
        { address: change.operator, label: 'Operator' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'STAKING_DEPOSIT':
    case 'STAKING_WITHDRAWAL':
    case 'STAKING_VOTE_LOCK':
    case 'POS_REGISTRATION':
    case 'POS_STAKE_INCREASE':
    case 'POS_RETIREMENT_REQUEST':
      return [{ address: change.account, label: 'Account' }];
    case 'GOVERNANCE_VOTE_CAST':
      return [{ address: change.voter, label: 'Voter' }];
    case 'SPONSORSHIP_FUNDING':
      return compactAddresses([
        { address: change.sponsor, label: 'Sponsor' },
        change.replacement
          ? {
              address: change.replacement.previousSponsor,
              label: 'Previous sponsor',
            }
          : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'CONTRACT_ADMIN_SET':
      return compactAddresses([
        change.admin ? { address: change.admin, label: 'Admin' } : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'SPONSORSHIP_ACCESS_RULE_SET':
      return compactAddresses([
        change.scope.type === 'ACCOUNT'
          ? { address: change.scope.address, label: 'Account' }
          : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'STORAGE_POINT_CONVERSION':
      return [{ address: change.contractAddress, label: 'Contract' }];
    case 'CROSS_SPACE_NATIVE_TRANSFER':
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
    case 'ESPACE':
      return getChangeAddresses(change.change).map((address) => ({
        ...address,
        label: `${address.label} / eSpace`,
      }));
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

function withAddressContext(
  endpoint: FlowEndpoint,
  context: string,
): FlowEndpoint {
  return endpoint.kind === 'address'
    ? { ...endpoint, context: endpoint.context ?? context }
    : endpoint;
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
