import {
  toChangeItemViewModel,
  type ChangeTone,
} from './changeView.ts';
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

type AssetMovementChange = Extract<
  SimulationChange,
  { changeType: 'BURN' | 'MINT' | 'TRANSFER' }
>;

export function isAssetFlowChange(change: SimulationChange) {
  return (
    change.changeType === 'TRANSFER' ||
    change.changeType === 'MINT' ||
    change.changeType === 'BURN' ||
    change.changeType === 'CROSS_SPACE_TRANSFER'
  );
}

export function toAssetFlowItemViewModel(
  change: SimulationChange,
  environmentId: EnvironmentId,
): AssetFlowItemViewModel | null {
  if (!isAssetFlowChange(change)) return null;

  const view = toChangeItemViewModel(change, environmentId);

  switch (change.changeType) {
    case 'TRANSFER':
      return {
        assetKey: movementAssetKey(change, environmentId),
        assetIdentifier: view.identifier,
        assetTitle: view.title,
        ...movementAmount(change),
        from: addressEndpoint('From', change.from),
        label: view.label,
        to: addressEndpoint('To', change.to),
        tone: view.tone,
        value: movementValue(change, view.value, view.title),
      };
    case 'MINT':
      return {
        assetKey: movementAssetKey(change, environmentId),
        assetIdentifier: view.identifier,
        assetTitle: view.title,
        ...movementAmount(change),
        from: { kind: 'terminal', label: 'Mint' },
        label: view.label,
        to: addressEndpoint('To', change.to),
        tone: view.tone,
        value: movementValue(change, view.value, view.title),
      };
    case 'BURN':
      return {
        assetKey: movementAssetKey(change, environmentId),
        assetIdentifier: view.identifier,
        assetTitle: view.title,
        ...movementAmount(change),
        from: addressEndpoint('From', change.from),
        label: view.label,
        to: { kind: 'terminal', label: 'Burn' },
        tone: view.tone,
        value: movementValue(change, view.value, view.title),
      };
    case 'CROSS_SPACE_TRANSFER':
      return {
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
      };
  }
}

export function getChangeAddresses(
  change: SimulationChange,
): ChangeAddressViewModel[] {
  switch (change.changeType) {
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
  change: AssetMovementChange,
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

function movementAmount(change: AssetMovementChange) {
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
  change: AssetMovementChange,
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
