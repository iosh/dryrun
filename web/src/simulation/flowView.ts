import {
  toChangeItemViewModel,
  type ChangeTone,
} from './changeView.ts';
import { formatHexQuantity } from '../lib/formatting.ts';
import type { AssetChange, ExecutionSpace } from './rpc.ts';
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
  decimals: number | null;
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
  switch (change.type) {
    case 'nativeTransfer':
      return [
        {
          assetKey: `NATIVE:${change.symbol}`,
          assetTitle: change.symbol,
          decimals: change.decimals,
          from: addressEndpoint('From', change.from, spaceLabel(change.space)),
          label: view.label,
          rawAmount: change.rawAmount,
          to: addressEndpoint('To', change.to, spaceLabel(change.space)),
          tone: view.tone,
          value: view.value ?? view.title,
        },
      ];
    case 'erc20Transfer':
      return [erc20Flow(change, view, change.from, change.to)];
    case 'erc20Mint':
      return [
        erc20Flow(
          change,
          view,
          { kind: 'terminal', label: 'Mint' },
          change.to,
        ),
      ];
    case 'erc20Burn':
      return [
        erc20Flow(
          change,
          view,
          change.from,
          { kind: 'terminal', label: 'Burn' },
        ),
      ];
    case 'erc721Transfer':
      return [erc721Flow(change, view, change.from, change.to)];
    case 'erc721Mint':
      return [
        erc721Flow(
          change,
          view,
          { kind: 'terminal', label: 'Mint' },
          change.to,
        ),
      ];
    case 'erc721Burn':
      return [
        erc721Flow(
          change,
          view,
          change.from,
          { kind: 'terminal', label: 'Burn' },
        ),
      ];
    case 'erc1155TransferSingle':
      return [erc1155SingleFlow(change, view, change.from, change.to)];
    case 'erc1155MintSingle':
      return [
        erc1155SingleFlow(
          change,
          view,
          { kind: 'terminal', label: 'Mint' },
          change.to,
        ),
      ];
    case 'erc1155BurnSingle':
      return [
        erc1155SingleFlow(
          change,
          view,
          change.from,
          { kind: 'terminal', label: 'Burn' },
        ),
      ];
    case 'erc1155TransferBatch':
      return erc1155BatchFlows(change, view, change.from, change.to);
    case 'erc1155MintBatch':
      return erc1155BatchFlows(
        change,
        view,
        { kind: 'terminal', label: 'Mint' },
        change.to,
      );
    case 'erc1155BurnBatch':
      return erc1155BatchFlows(
        change,
        view,
        change.from,
        { kind: 'terminal', label: 'Burn' },
      );
    case 'selfDestructBurn':
      return [{
        assetKey: `NATIVE:${change.symbol}`,
        assetTitle: change.symbol,
        decimals: change.decimals,
        from: addressEndpoint('Contract', change.contractAddress, spaceLabel(change.space)),
        label: view.label,
        rawAmount: change.rawAmount,
        to: { kind: 'terminal', label: 'Burn' },
        tone: view.tone,
        value: view.value ?? view.title,
      }];
    case 'crossSpaceNativeTransfer':
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
    case 'stakingDeposit':
    case 'stakingWithdrawal':
    case 'stakingVoteLock':
    case 'posRegistration':
    case 'posStakeIncrease':
    case 'posRetirementRequest':
      return [{ address: change.account, label: 'Account' }];
    case 'governanceVoteCast':
      return [{ address: change.voter, label: 'Voter' }];
    case 'sponsorshipFunding':
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
    case 'contractAdminSet':
      return compactAddresses([
        change.admin ? { address: change.admin, label: 'Admin' } : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'sponsorshipAccessRule':
    case 'sponsorshipAccessRuleSet':
      return compactAddresses([
        change.scope.type === 'account'
          ? { address: change.scope.address, label: 'Account' }
          : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'storageCollateral':
    case 'storagePointConversion':
      return [{ address: change.contractAddress, label: 'Contract' }];
    case 'crossSpaceNativeTransfer':
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
    case 'gasSponsorship':
    case 'storageSponsorship':
      return compactAddresses([
        change.sponsor ? { address: change.sponsor, label: 'Sponsor' } : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
    case 'contractAdmin':
      return compactAddresses([
        change.state?.admin ? { address: change.state.admin, label: 'Admin' } : null,
        { address: change.contractAddress, label: 'Contract' },
      ]);
  }
}

function erc20Flow(
  change: Extract<AssetChange, { type: 'erc20Transfer' | 'erc20Mint' | 'erc20Burn' }>,
  view: ReturnType<typeof toChangeItemViewModel>,
  from: string | FlowEndpoint,
  to: string | FlowEndpoint,
): AssetFlowItemViewModel {
  return {
    assetKey: `ERC20:${normalizeAddress(change.contractAddress)}`,
    assetIdentifier: change.contractAddress,
    assetTitle: view.title,
    decimals: change.decimals ?? null,
    from: flowEndpoint('From', from, spaceLabel(change.space)),
    label: view.label,
    rawAmount: change.rawAmount,
    to: flowEndpoint('To', to, spaceLabel(change.space)),
    tone: view.tone,
    value: view.value ?? view.title,
  };
}

function erc721Flow(
  change: Extract<AssetChange, { type: 'erc721Transfer' | 'erc721Mint' | 'erc721Burn' }>,
  view: ReturnType<typeof toChangeItemViewModel>,
  from: string | FlowEndpoint,
  to: string | FlowEndpoint,
): AssetFlowItemViewModel {
  return {
    assetKey: `ERC721:${normalizeAddress(change.contractAddress)}:${change.tokenId}`,
    assetIdentifier: change.contractAddress,
    assetTitle: view.title,
    decimals: 0,
    from: flowEndpoint('From', from, spaceLabel(change.space)),
    label: view.label,
    rawAmount: '0x1',
    to: flowEndpoint('To', to, spaceLabel(change.space)),
    tone: view.tone,
    value: view.title,
  };
}

function erc1155SingleFlow(
  change: Extract<
    AssetChange,
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
    from: flowEndpoint('From', from, spaceLabel(change.space)),
    label: view.label,
    rawAmount: change.rawAmount,
    to: flowEndpoint('To', to, spaceLabel(change.space)),
    tone: view.tone,
    value: `${formatHexQuantity(change.rawAmount)} ${assetTitle}`,
  };
}

function erc1155BatchFlows(
  change: Extract<
    AssetChange,
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
      from: flowEndpoint('From', from, spaceLabel(change.space)),
      label: view.label,
      rawAmount: item.rawAmount,
      to: flowEndpoint('To', to, spaceLabel(change.space)),
      tone: view.tone,
      value: `${formatHexQuantity(item.rawAmount)} ${assetTitle}`,
    };
  });
}

function flowEndpoint(
  label: string,
  endpoint: string | FlowEndpoint,
  context?: string,
): FlowEndpoint {
  return typeof endpoint === 'string' ? addressEndpoint(label, endpoint, context) : endpoint;
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

function compactAddresses(
  addresses: Array<ChangeAddressViewModel | null>,
) {
  return addresses.filter(
    (address): address is ChangeAddressViewModel => address !== null,
  );
}

export function spaceLabel(space?: ExecutionSpace | 'coreSpace') {
  if (space === 'core' || space === 'coreSpace') return 'Core';
  if (space === 'espace') return 'eSpace';
  return undefined;
}
