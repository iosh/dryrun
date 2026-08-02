import { formatAmount, formatNativeAmount } from '../../../lib/formatting.ts';
import { getEnvironment } from '../../environment.ts';
import {
  isAssetFlowChange,
  normalizeAddress,
  toAssetFlowItemViewModel,
  type AssetFlowItemViewModel,
  type ChangeAddressViewModel,
  type FlowEndpoint,
} from '../../flowView.ts';
import type { SimulationRecord } from '../../types.ts';
import type {
  ExecutionAnchor,
  FlowLaneViewModel,
  FlowSegment,
  SenderImpactItem,
  SimulationResultViewModel,
} from './resultTypes.ts';

export function createSimulationResultViewModel(
  record: SimulationRecord,
): SimulationResultViewModel {
  const environment = getEnvironment(record.environmentId);
  const changes = record.response.changes;
  const flowItems = changes.flatMap((change, changeIndex) => {
    const item = toAssetFlowItemViewModel(change, record.environmentId);
    return item ? [{ ...item, changeIndex }] : [];
  });
  const stateEffects = changes.filter(
    (change) => !isAssetFlowChange(change),
  );

  return {
    anchor: executionAnchor(record),
    changes,
    environment,
    flowItems,
    lanes: buildFlowLanes(record, flowItems),
    senderImpacts: senderNetImpacts(
      flowItems,
      record.request.transaction.from,
      formatNativeAmount(
        record.response.execution.fee,
        environment.nativeSymbol,
      ),
    ),
    stateEffects,
  };
}

export function buildParticipantMap(lanes: readonly FlowLaneViewModel[]) {
  return new Map(
    lanes.flatMap((lane) =>
      lane.kind === 'address'
        ? [[normalizeAddress(lane.address), lane] as const]
        : [],
    ),
  );
}

export function flowEndpointLaneKey(endpoint: FlowEndpoint) {
  if (endpoint.kind === 'terminal') {
    return `terminal:${endpoint.label}`;
  }
  const context = endpoint.context ?? inferAddressContext(endpoint.address);
  return `address:${context.toLowerCase()}:${normalizeAddress(endpoint.address)}`;
}

function inferAddressContext(address: string) {
  return address.includes(':') ? 'Core' : '';
}

export function flowItemAddresses(item: AssetFlowItemViewModel) {
  const addresses: ChangeAddressViewModel[] = [];
  if (item.from.kind === 'address') {
    addresses.push({ address: item.from.address, label: item.from.label });
  }
  if (item.to.kind === 'address') {
    addresses.push({ address: item.to.address, label: item.to.label });
  }
  if (item.assetIdentifier) {
    addresses.push({
      address: item.assetIdentifier,
      label: 'Asset contract',
    });
  }
  return addresses;
}

export function hasHighlightedAddress(
  addresses: readonly ChangeAddressViewModel[],
  activeAddress: string | null,
) {
  return (
    activeAddress !== null &&
    addresses.some(
      (item) => normalizeAddress(item.address) === activeAddress,
    )
  );
}

export function getFlowSegment(
  index: number,
  fromIndex: number,
  toIndex: number,
): FlowSegment {
  if (fromIndex === toIndex) return null;
  const lowIndex = Math.min(fromIndex, toIndex);
  const highIndex = Math.max(fromIndex, toIndex);
  if (index < lowIndex || index > highIndex) return null;
  if (index === fromIndex) {
    return fromIndex < toIndex ? 'right-half' : 'left-half';
  }
  if (index === toIndex) {
    return fromIndex < toIndex ? 'left-half' : 'right-half';
  }
  return 'full';
}

export function changeAddressLabel(
  item: ChangeAddressViewModel,
  record: SimulationRecord,
) {
  const role = transactionAddressRole(item.address, record);
  if (!role) return item.label;
  if (role === 'Sender' && item.label === 'From') return role;
  if (role === 'Target' && item.label === 'To') return role;
  return `${role} / ${item.label}`;
}

function executionAnchor(record: SimulationRecord): ExecutionAnchor {
  const execution = record.response.execution;
  return 'block' in execution
    ? {
        hash: execution.block.hash,
        label: 'Block',
        number: execution.block.number,
      }
    : {
        hash: execution.state.pivotHash,
        label: 'Epoch',
        number: execution.state.epochNumber,
      };
}

function senderNetImpacts(
  flowItems: readonly AssetFlowItemViewModel[],
  sender: string,
  fee: string,
): SenderImpactItem[] {
  const normalizedSender = normalizeAddress(sender);
  const balances = new Map<
    string,
    { amount: bigint; item: AssetFlowItemViewModel }
  >();

  for (const item of flowItems) {
    let amount = 0n;
    if (
      item.from.kind === 'address' &&
      normalizeAddress(item.from.address) === normalizedSender
    ) {
      amount -= BigInt(item.rawAmount);
    }
    if (
      item.to.kind === 'address' &&
      normalizeAddress(item.to.address) === normalizedSender
    ) {
      amount += BigInt(item.rawAmount);
    }
    if (amount === 0n) continue;

    const balance = balances.get(item.assetKey);
    if (balance) {
      balance.amount += amount;
    } else {
      balances.set(item.assetKey, { amount, item });
    }
  }

  const impacts: SenderImpactItem[] = [];
  for (const balance of balances.values()) {
    if (balance.amount === 0n) continue;
    const received = balance.amount > 0n;
    impacts.push({
      label: received ? 'Received' : 'Sent',
      tone: received ? 'positive' : 'negative',
      value: formatSenderNetValue(balance.amount, balance.item),
    });
  }
  impacts.push({ label: 'Fee', tone: 'neutral', value: fee });
  return impacts;
}

function formatSenderNetValue(
  amount: bigint,
  item: AssetFlowItemViewModel,
) {
  const absoluteAmount = amount < 0n ? -amount : amount;
  const value = formatAmount(absoluteAmount, item.decimals, item.assetTitle);
  return `${amount > 0n ? '+' : '-'}${value}`;
}

function buildFlowLanes(
  record: SimulationRecord,
  flowItems: readonly AssetFlowItemViewModel[],
) {
  const lanes: FlowLaneViewModel[] = [];
  const knownLanes = new Set<string>();
  let nextAliasIndex = 0;

  const addAddress = (address: string, alias?: string, context?: string) => {
    const endpoint: FlowEndpoint = {
      address,
      context,
      kind: 'address',
      label: alias ?? 'Address',
    };
    const key = flowEndpointLaneKey(endpoint);
    if (knownLanes.has(key)) return;
    knownLanes.add(key);
    lanes.push({
      address,
      alias:
        alias ??
        transactionAddressRole(address, record) ??
        participantAlias(nextAliasIndex++),
      context,
      key,
      kind: 'address',
    });
  };

  const addEndpoint = (endpoint: FlowEndpoint) => {
    if (endpoint.kind === 'address') {
      addAddress(endpoint.address, undefined, endpoint.context);
      return;
    }

    const key = flowEndpointLaneKey(endpoint);
    if (knownLanes.has(key)) return;
    knownLanes.add(key);
    lanes.push({ alias: endpoint.label, key, kind: 'terminal' });
  };

  addAddress(record.request.transaction.from, 'Sender');
  if (record.request.transaction.to) {
    addAddress(record.request.transaction.to, 'Target');
  }

  for (const item of flowItems) {
    addEndpoint(item.from);
    addEndpoint(item.to);
  }

  return lanes;
}

function participantAlias(index: number) {
  const letter = String.fromCharCode(65 + (index % 26));
  const cycle = Math.floor(index / 26);
  return cycle === 0 ? letter : `${letter}${cycle + 1}`;
}

function transactionAddressRole(
  address: string,
  record: SimulationRecord,
) {
  const normalized = normalizeAddress(address);
  if (normalized === normalizeAddress(record.request.transaction.from)) {
    return 'Sender';
  }
  if (
    record.request.transaction.to &&
    normalized === normalizeAddress(record.request.transaction.to)
  ) {
    return 'Target';
  }
  return undefined;
}
