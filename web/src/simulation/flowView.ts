import {
  toChangeItemViewModel,
  type ChangeTone,
} from './changeView.ts';
import { formatHexQuantity } from '../lib/formatting.ts';
import type { EvmChange } from './rpc.ts';
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
  if ('type' in change) {
    return toEvmAssetFlowItemViewModels(change, view);
  }

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
  if ('type' in change) {
    return getEvmChangeAddresses(change);
  }

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

function toEvmAssetFlowItemViewModels(
  change: EvmChange,
  view: ReturnType<typeof toChangeItemViewModel>,
): AssetFlowItemViewModel[] {
  switch (change.type) {
    case 'nativeTransfer':
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
    case 'erc20Transfer':
      return [evmErc20Flow(change, view, change.from, change.to)];
    case 'erc20Mint':
      return [
        evmErc20Flow(
          change,
          view,
          { kind: 'terminal', label: 'Mint' },
          change.to,
        ),
      ];
    case 'erc20Burn':
      return [
        evmErc20Flow(
          change,
          view,
          change.from,
          { kind: 'terminal', label: 'Burn' },
        ),
      ];
    case 'erc721Transfer':
      return [evmErc721Flow(change, view, change.from, change.to)];
    case 'erc721Mint':
      return [
        evmErc721Flow(
          change,
          view,
          { kind: 'terminal', label: 'Mint' },
          change.to,
        ),
      ];
    case 'erc721Burn':
      return [
        evmErc721Flow(
          change,
          view,
          change.from,
          { kind: 'terminal', label: 'Burn' },
        ),
      ];
    case 'erc1155TransferSingle':
      return [evmErc1155SingleFlow(change, view, change.from, change.to)];
    case 'erc1155MintSingle':
      return [
        evmErc1155SingleFlow(
          change,
          view,
          { kind: 'terminal', label: 'Mint' },
          change.to,
        ),
      ];
    case 'erc1155BurnSingle':
      return [
        evmErc1155SingleFlow(
          change,
          view,
          change.from,
          { kind: 'terminal', label: 'Burn' },
        ),
      ];
    case 'erc1155TransferBatch':
      return evmErc1155BatchFlows(change, view, change.from, change.to);
    case 'erc1155MintBatch':
      return evmErc1155BatchFlows(
        change,
        view,
        { kind: 'terminal', label: 'Mint' },
        change.to,
      );
    case 'erc1155BurnBatch':
      return evmErc1155BatchFlows(
        change,
        view,
        change.from,
        { kind: 'terminal', label: 'Burn' },
      );
    default:
      return [];
  }
}

function evmErc20Flow(
  change: Extract<EvmChange, { type: 'erc20Transfer' | 'erc20Mint' | 'erc20Burn' }>,
  view: ReturnType<typeof toChangeItemViewModel>,
  from: string | FlowEndpoint,
  to: string | FlowEndpoint,
): AssetFlowItemViewModel {
  return {
    assetKey: `ERC20:${normalizeAddress(change.contractAddress)}`,
    assetIdentifier: change.contractAddress,
    assetTitle: view.title,
    decimals: change.decimals ?? 0,
    from: flowEndpoint('From', from),
    label: view.label,
    rawAmount: change.rawAmount,
    to: flowEndpoint('To', to),
    tone: view.tone,
    value: view.value ?? view.title,
  };
}

function evmErc721Flow(
  change: Extract<EvmChange, { type: 'erc721Transfer' | 'erc721Mint' | 'erc721Burn' }>,
  view: ReturnType<typeof toChangeItemViewModel>,
  from: string | FlowEndpoint,
  to: string | FlowEndpoint,
): AssetFlowItemViewModel {
  return {
    assetKey: `ERC721:${normalizeAddress(change.contractAddress)}:${change.tokenId}`,
    assetIdentifier: change.contractAddress,
    assetTitle: view.title,
    decimals: 0,
    from: flowEndpoint('From', from),
    label: view.label,
    rawAmount: '0x1',
    to: flowEndpoint('To', to),
    tone: view.tone,
    value: view.title,
  };
}

function evmErc1155SingleFlow(
  change: Extract<
    EvmChange,
    { type: 'erc1155TransferSingle' | 'erc1155MintSingle' | 'erc1155BurnSingle' }
  >,
  view: ReturnType<typeof toChangeItemViewModel>,
  from: string | FlowEndpoint,
  to: string | FlowEndpoint,
): AssetFlowItemViewModel {
  const assetTitle = `ERC-1155 #${formatHexQuantity(change.tokenId)}`;
  return {
    assetKey: `ERC1155:${normalizeAddress(change.contractAddress)}:${change.tokenId}`,
    assetIdentifier: change.contractAddress,
    assetTitle,
    decimals: 0,
    from: flowEndpoint('From', from),
    label: view.label,
    rawAmount: change.rawAmount,
    to: flowEndpoint('To', to),
    tone: view.tone,
    value: `${formatHexQuantity(change.rawAmount)} ${assetTitle}`,
  };
}

function evmErc1155BatchFlows(
  change: Extract<
    EvmChange,
    { type: 'erc1155TransferBatch' | 'erc1155MintBatch' | 'erc1155BurnBatch' }
  >,
  view: ReturnType<typeof toChangeItemViewModel>,
  from: string | FlowEndpoint,
  to: string | FlowEndpoint,
): AssetFlowItemViewModel[] {
  return change.items.map((item) => {
    const assetTitle = `ERC-1155 #${formatHexQuantity(item.tokenId)}`;
    return {
      assetKey: `ERC1155:${normalizeAddress(change.contractAddress)}:${item.tokenId}`,
      assetIdentifier: change.contractAddress,
      assetTitle,
      decimals: 0,
      from: flowEndpoint('From', from),
      label: view.label,
      rawAmount: item.rawAmount,
      to: flowEndpoint('To', to),
      tone: view.tone,
      value: `${formatHexQuantity(item.rawAmount)} ${assetTitle}`,
    };
  });
}

function flowEndpoint(label: string, endpoint: string | FlowEndpoint): FlowEndpoint {
  return typeof endpoint === 'string' ? addressEndpoint(label, endpoint) : endpoint;
}

function getEvmChangeAddresses(change: EvmChange): ChangeAddressViewModel[] {
  switch (change.type) {
    case 'nativeTransfer':
      return [
        { address: change.from, label: 'From' },
        { address: change.to, label: 'To' },
      ];
    case 'selfDestructBurn':
      return [{ address: change.contractAddress, label: 'Contract' }];
    case 'accountDelegation':
      return [{ address: change.account, label: 'Account' }];
    case 'wrappedNativeDeposit':
    case 'wrappedNativeWithdrawal':
      return [
        { address: change.account, label: 'Account' },
        { address: change.contractAddress, label: 'Wrapper contract' },
      ];
    case 'erc20Transfer':
    case 'erc721Transfer':
      return [
        { address: change.from, label: 'From' },
        { address: change.to, label: 'To' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'erc20Mint':
    case 'erc721Mint':
      return [
        { address: change.to, label: 'To' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'erc20Burn':
    case 'erc721Burn':
      return [
        { address: change.from, label: 'From' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'erc1155TransferSingle':
    case 'erc1155TransferBatch':
      return [
        { address: change.from, label: 'From' },
        { address: change.to, label: 'To' },
        { address: change.operator, label: 'Operator' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'erc1155MintSingle':
    case 'erc1155MintBatch':
      return [
        { address: change.to, label: 'To' },
        { address: change.operator, label: 'Operator' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'erc1155BurnSingle':
    case 'erc1155BurnBatch':
      return [
        { address: change.from, label: 'From' },
        { address: change.operator, label: 'Operator' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'erc20Approval':
      return [
        { address: change.owner, label: 'Owner' },
        { address: change.spender, label: 'Spender' },
        { address: change.contractAddress, label: 'Asset contract' },
      ];
    case 'erc721Approval':
      return compactAddresses([
        { address: change.owner, label: 'Owner' },
        change.before ? { address: change.before, label: 'Previous approval' } : null,
        change.after ? { address: change.after, label: 'Approved address' } : null,
        { address: change.contractAddress, label: 'Asset contract' },
      ]);
    case 'operatorApproval':
      return [
        { address: change.owner, label: 'Owner' },
        { address: change.operator, label: 'Operator' },
        { address: change.contractAddress, label: 'Asset contract' },
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
