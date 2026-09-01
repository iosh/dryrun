import { formatAmount, formatNativeAmount } from '../../../lib/formatting.ts';
import { getEnvironment } from '../../environment.ts';
import type { EvmOutcome } from '../../rpc.ts';
import {
  normalizeAddress,
  toAssetFlowItemViewModels,
  type AssetFlowItemViewModel,
  type ChangeAddressViewModel,
  type FlowEndpoint,
} from '../../flowView.ts';
import {
  simulationChanges,
  type SimulationRecord,
} from '../../types.ts';
import type {
  ExecutionAnchor,
  FlowLaneViewModel,
  FlowSegment,
  SenderImpactItem,
  SimulationExecution,
  SimulationResultViewModel,
} from './resultTypes.ts';

export function createSimulationResultViewModel(
  record: SimulationRecord,
): SimulationResultViewModel {
  const environment = getEnvironment(record.environmentId);
  const { error: changesError, items: changes } = simulationChanges(
    record.response,
  );
  const changeFlows = changes.map((change) => ({
    change,
    items: toAssetFlowItemViewModels(change),
  }));
  const flowItems = changeFlows
    .flatMap(({ items }, changeIndex) =>
      items.map((item) => ({
        ...item,
        changeIndex,
      })),
    )
    .map((item, flowIndex) => ({ ...item, flowIndex }));
  const stateEffects = changeFlows.flatMap(({ change, items }) =>
    items.length === 0 ? [change] : [],
  );
  const execution = simulationExecution(record);

  return {
    anchor: executionAnchor(record),
    changes,
    changesError,
    environment,
    execution,
    flowItems,
    lanes: buildFlowLanes(record, flowItems),
    senderImpacts: senderNetImpacts(
      flowItems,
      record.request.transaction.from,
      execution.totalFee === null
        ? null
        : formatNativeAmount(execution.totalFee, environment.nativeSymbol),
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
  if ('outcome' in record.response) {
    return {
      hash: record.response.state.blockHash,
      label: 'Block',
      number: record.response.state.blockNumber,
    };
  }

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
  fee: string | null,
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
  if (fee !== null) {
    impacts.push({ label: 'Fee', tone: 'neutral', value: fee });
  }
  return impacts;
}

function simulationExecution(record: SimulationRecord): SimulationExecution {
  const response = record.response;
  if ('outcome' in response) {
    const outcome = response.outcome;
    const executed = 'gasUsed' in outcome;
    const blobGasFee = executed ? outcome.blobGasFee ?? null : null;
    const gasFee = executed ? outcome.gasFee : null;
    return {
      blobGasFee,
      blobGasPrice: executed ? outcome.blobGasPrice ?? null : null,
      blobGasUsed: executed ? outcome.blobGasUsed ?? null : null,
      burntGasFee: executed ? outcome.burntGasFee ?? null : null,
      chainId: response.transaction.chainId,
      contractAddress:
        outcome.status === 'success' && 'contractAddress' in outcome
          ? outcome.contractAddress
          : null,
      effectiveGasPrice: executed ? outcome.effectiveGasPrice : null,
      failure: evmFailure(outcome),
      gasCharged: null,
      gasCoveredBySponsor: null,
      gasFee,
      gasLimit: response.transaction.gas,
      gasUsed: executed ? outcome.gasUsed : null,
      logsCount: outcome.status === 'success' ? outcome.logs.length : 0,
      output: evmOutput(outcome),
      status: outcome.status,
      storageCoveredBySponsor: null,
      totalFee: gasFee === null ? null : addHexQuantities(gasFee, blobGasFee),
    };
  }

  const execution = response.execution;
  return {
    blobGasFee: null,
    blobGasPrice: null,
    blobGasUsed: null,
    burntGasFee: execution.burntFee,
    chainId: execution.chainId,
    contractAddress: null,
    effectiveGasPrice: null,
    failure: execution.failure
      ? {
          detail: [execution.failure.code, execution.failure.reason]
            .filter(Boolean)
            .join(' / '),
          message: execution.failure.message,
        }
      : null,
    gasCharged: 'gasCharged' in execution ? execution.gasCharged : null,
    gasCoveredBySponsor:
      'gasCoveredBySponsor' in execution
        ? execution.gasCoveredBySponsor
        : null,
    gasFee: execution.fee,
    gasLimit: execution.gasLimit,
    gasUsed: execution.gasUsed,
    logsCount: 0,
    output: { label: 'Output', value: execution.output },
    status:
      execution.status === 'SUCCESS'
        ? 'success'
        : execution.status === 'FAILED'
          ? 'failed'
          : 'rejected',
    storageCoveredBySponsor:
      'storageCoveredBySponsor' in execution
        ? execution.storageCoveredBySponsor
        : null,
    totalFee: execution.fee,
  };
}

function evmFailure(outcome: EvmOutcome) {
  switch (outcome.status) {
    case 'reverted':
      return {
        detail: outcome.reason,
        message: 'Execution reverted',
      };
    case 'failed':
    case 'rejected':
      return { message: outcome.error };
    case 'success':
      return null;
  }
}

function evmOutput(outcome: EvmOutcome) {
  if (outcome.status === 'reverted') {
    return { label: 'Revert data', value: outcome.revertData };
  }
  if (outcome.status !== 'success') return null;
  return 'returnData' in outcome
    ? { label: 'Return data', value: outcome.returnData }
    : { label: 'Runtime code', value: outcome.runtimeCode };
}

function addHexQuantities(left: string, right: string | null) {
  return `0x${(BigInt(left) + (right === null ? 0n : BigInt(right))).toString(16)}`;
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
