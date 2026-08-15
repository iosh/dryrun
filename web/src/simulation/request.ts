import {
  getAddress as getCoreAddress,
  isAddress as isCoreAddress,
} from 'cive/utils';
import { parseCFX, parseGDrip } from 'cive';
import {
  getAddress,
  isAddress,
  parseEther,
  parseGwei,
  toHex,
} from 'viem';

import { getEnvironment, type EnvironmentId } from './environment.ts';
import type {
  CoreSimulationRequest,
  CoreTransactionRequest,
  HexSimulationRequest,
  HexTransactionRequest,
  ParsedFormResult,
  RpcAccessListItem,
  SimulationFormValues,
  SimulationRequest,
  ContextMode,
  TxTypeOption,
} from './types.ts';

const TX_TYPE_TO_HEX: Record<Exclude<TxTypeOption, 'auto'>, string> = {
  legacy: '0x0',
  'access-list': '0x1',
  'dynamic-fee': '0x2',
};

const STORAGE_KEY_PATTERN = /^0x[0-9a-fA-F]{64}$/;
const DATA_PATTERN = /^0x(?:[0-9a-fA-F]{2})*$/;
const BLOCK_HASH_PATTERN = /^0x[0-9a-fA-F]{64}$/;

interface ParseSuccess<T> {
  ok: true;
  value: T;
}

interface ParseFailure {
  ok: false;
  issue: string;
}

type ParseResult<T> = ParseSuccess<T> | ParseFailure;
type EffectiveTxType = Exclude<TxTypeOption, 'auto'>;

interface ParsedValues {
  from?: string;
  to?: string;
  value?: string;
  data?: string;
  contextNumber?: string;
  nonce?: string;
  gasLimit?: string;
  gasPrice?: string;
  maxFeePerGas?: string;
  maxPriorityFeePerGas?: string;
  accessList?: RpcAccessListItem[];
  storageLimit?: string;
  epochHeight?: string;
}

export function createInitialFormValues(): SimulationFormValues {
  return {
    from: '',
    to: '',
    value: '',
    data: '',
    contextMode: 'latest',
    contextNumber: '',
    nonce: '',
    gasLimit: '',
    txType: 'auto',
    gasPrice: '',
    maxFeePerGas: '',
    maxPriorityFeePerGas: '',
    accessListJson: '',
    storageLimit: '',
    epochHeight: '',
  };
}

export function parseSimulationForm(
  environmentId: EnvironmentId,
  values: SimulationFormValues,
): ParsedFormResult {
  const fieldIssues: ParsedFormResult['fieldIssues'] = {};
  const parsed: ParsedValues = {};

  parsed.from = readParsed(
    fieldIssues,
    'from',
    parseAddress(environmentId, values.from, 'From address', true),
  );
  parsed.to = readParsed(
    fieldIssues,
    'to',
    parseAddress(environmentId, values.to, 'To address', false),
  );
  parsed.value = readParsed(
    fieldIssues,
    'value',
    parseValue(environmentId, values.value),
  );
  parsed.data = readParsed(fieldIssues, 'data', parseData(values.data));
  parsed.contextNumber = readParsed(
    fieldIssues,
    'contextNumber',
    values.contextMode === 'number'
      ? parseRequiredQuantity(values.contextNumber, contextValueLabel(values.contextMode, environmentId))
      : values.contextMode === 'hash'
        ? parseRequiredBlockHash(values.contextNumber)
        : success(undefined),
  );
  parsed.nonce = readParsed(
    fieldIssues,
    'nonce',
    parseOptionalQuantity(values.nonce, 'Nonce'),
  );
  parsed.gasLimit = readParsed(
    fieldIssues,
    'gasLimit',
    parseOptionalQuantity(values.gasLimit, 'Gas limit'),
  );
  parsed.gasPrice = readParsed(
    fieldIssues,
    'gasPrice',
    parseFee(environmentId, values.gasPrice, 'Gas price'),
  );
  parsed.maxFeePerGas = readParsed(
    fieldIssues,
    'maxFeePerGas',
    parseFee(environmentId, values.maxFeePerGas, 'Max fee per gas'),
  );
  parsed.maxPriorityFeePerGas = readParsed(
    fieldIssues,
    'maxPriorityFeePerGas',
    parseFee(
      environmentId,
      values.maxPriorityFeePerGas,
      'Max priority fee per gas',
    ),
  );
  parsed.accessList = readParsed(
    fieldIssues,
    'accessListJson',
    parseAccessList(environmentId, values.accessListJson),
  );

  if (environmentId === 'conflux-core-mainnet') {
    parsed.storageLimit = readParsed(
      fieldIssues,
      'storageLimit',
      parseOptionalQuantity(values.storageLimit, 'Storage limit'),
    );
    parsed.epochHeight = readParsed(
      fieldIssues,
      'epochHeight',
      parseOptionalQuantity(values.epochHeight, 'Epoch height'),
    );
  }

  const formIssues = validateRelationships(environmentId, values, parsed);

  if (
    Object.keys(fieldIssues).length > 0 ||
    formIssues.length > 0 ||
    !parsed.from
  ) {
    return { fieldIssues, formIssues };
  }

  return {
    fieldIssues,
    formIssues,
    request: buildRequest(environmentId, values, {
      ...parsed,
      from: parsed.from,
    }),
  };
}

export function validateSimulationField(
  environmentId: EnvironmentId,
  field: keyof SimulationFormValues,
  value: string,
): string | undefined {
  const result = (() => {
    switch (field) {
      case 'from':
        return parseAddress(environmentId, value, 'From address', true);
      case 'to':
        return parseAddress(environmentId, value, 'To address', false);
      case 'value':
        return parseValue(environmentId, value);
      case 'data':
        return parseData(value);
      case 'contextNumber':
        return environmentId === 'conflux-espace-mainnet'
          ? parseOptionalBlockReference(value)
          : parseOptionalQuantity(value, contextNumberLabel(environmentId));
      case 'nonce':
        return parseOptionalQuantity(value, 'Nonce');
      case 'gasLimit':
        return parseOptionalQuantity(value, 'Gas limit');
      case 'gasPrice':
        return parseFee(environmentId, value, 'Gas price');
      case 'maxFeePerGas':
        return parseFee(environmentId, value, 'Max fee per gas');
      case 'maxPriorityFeePerGas':
        return parseFee(environmentId, value, 'Max priority fee per gas');
      case 'accessListJson':
        return parseAccessList(environmentId, value);
      case 'storageLimit':
        return parseOptionalQuantity(value, 'Storage limit');
      case 'epochHeight':
        return parseOptionalQuantity(value, 'Epoch height');
      case 'contextMode':
      case 'txType':
        return success(undefined);
      default:
        return assertNever(field);
    }
  })();

  return result.ok ? undefined : result.issue;
}

export function countAdvancedValues(
  environmentId: EnvironmentId,
  values: SimulationFormValues,
): number {
  const dynamicFeeEnabled =
    values.txType === 'auto' ||
    values.txType === 'dynamic-fee';
  const accessListEnabled = values.txType !== 'legacy';

  return [
    values.contextMode !== 'latest' ? values.contextMode : '',
    values.contextMode === 'number' || values.contextMode === 'hash'
      ? values.contextNumber
      : '',
    values.nonce,
    values.gasLimit,
    values.txType !== 'auto' ? values.txType : '',
    values.txType !== 'dynamic-fee' ? values.gasPrice : '',
    dynamicFeeEnabled ? values.maxFeePerGas : '',
    dynamicFeeEnabled ? values.maxPriorityFeePerGas : '',
    accessListEnabled && values.accessListJson.trim() !== '[]'
      ? values.accessListJson
      : '',
    environmentId === 'conflux-core-mainnet' ? values.storageLimit : '',
    environmentId === 'conflux-core-mainnet' ? values.epochHeight : '',
  ].filter((value) => value.trim().length > 0).length;
}

function buildRequest(
  environmentId: EnvironmentId,
  values: SimulationFormValues,
  parsed: ParsedValues & { from: string },
): SimulationRequest {
  const environment = getEnvironment(environmentId);
  const transaction: HexTransactionRequest = {
    chainId: toHex(environment.chainId),
    from: parsed.from,
    ...(parsed.to ? { to: parsed.to } : {}),
    ...(parsed.nonce ? { nonce: parsed.nonce } : {}),
    ...(parsed.gasLimit ? { gas: parsed.gasLimit } : {}),
    ...(parsed.value ? { value: parsed.value } : {}),
    ...(parsed.data ? { data: parsed.data } : {}),
    ...(parsed.accessList && parsed.accessList.length > 0
      ? { accessList: parsed.accessList }
      : {}),
    ...(parsed.gasPrice ? { gasPrice: parsed.gasPrice } : {}),
    ...(parsed.maxFeePerGas ? { maxFeePerGas: parsed.maxFeePerGas } : {}),
    ...(parsed.maxPriorityFeePerGas
      ? { maxPriorityFeePerGas: parsed.maxPriorityFeePerGas }
      : {}),
    ...(values.txType !== 'auto'
      ? { type: TX_TYPE_TO_HEX[values.txType] }
      : {}),
  };

  if (environmentId === 'conflux-core-mainnet') {
    const coreTransaction: CoreTransactionRequest = {
      ...transaction,
      ...(parsed.storageLimit ? { storageLimit: parsed.storageLimit } : {}),
      ...(parsed.epochHeight ? { epochHeight: parsed.epochHeight } : {}),
    };
    const request: CoreSimulationRequest = {
      epoch:
        values.contextMode === 'number'
          ? parsed.contextNumber!
          : 'latest_state',
      transaction: coreTransaction,
    };
    return request;
  }

  const request: HexSimulationRequest = {
    block:
      values.contextMode === 'number' || values.contextMode === 'hash'
        ? parsed.contextNumber!
        : values.contextMode,
    transaction,
  };
  return request;
}

function validateRelationships(
  environmentId: EnvironmentId,
  values: SimulationFormValues,
  parsed: ParsedValues,
): string[] {
  const issues: string[] = [];
  const effectiveTxType = resolveEffectiveTxType(values.txType, parsed);

  if (
    environmentId !== 'ethereum-mainnet' &&
    (values.contextMode === 'safe' || values.contextMode === 'finalized')
  ) {
    issues.push(
      environmentId === 'conflux-espace-mainnet'
        ? 'Conflux eSpace supports Latest, Number, or Hash.'
        : 'This environment only supports Latest or a specific number.',
    );
  }

  if (values.contextMode === 'hash' && environmentId !== 'conflux-espace-mainnet') {
    issues.push('Block hash selection is only available for Conflux eSpace.');
  }

  if (parsed.gasPrice && (parsed.maxFeePerGas || parsed.maxPriorityFeePerGas)) {
    issues.push('Gas price cannot be combined with dynamic fee fields.');
  }

  if (
    effectiveTxType === 'legacy' &&
    (parsed.maxFeePerGas || parsed.maxPriorityFeePerGas)
  ) {
    issues.push('Legacy transactions cannot include dynamic fee fields.');
  }

  if (effectiveTxType === 'dynamic-fee' && parsed.gasPrice) {
    issues.push('Dynamic fee transactions cannot include a gas price.');
  }

  if (
    effectiveTxType === 'legacy' &&
    parsed.accessList &&
    parsed.accessList.length > 0
  ) {
    issues.push('Legacy transactions cannot include an access list.');
  }

  if (
    effectiveTxType === 'access-list' &&
    (parsed.maxFeePerGas || parsed.maxPriorityFeePerGas)
  ) {
    issues.push('Access list transactions cannot include dynamic fee fields.');
  }

  if (parsed.maxFeePerGas && parsed.maxPriorityFeePerGas) {
    if (BigInt(parsed.maxPriorityFeePerGas) > BigInt(parsed.maxFeePerGas)) {
      issues.push('Max priority fee cannot exceed max fee per gas.');
    }
  }

  return issues;
}

function resolveEffectiveTxType(
  selectedType: TxTypeOption,
  parsed: ParsedValues,
): EffectiveTxType {
  if (selectedType !== 'auto') return selectedType;
  if (parsed.maxFeePerGas || parsed.maxPriorityFeePerGas) {
    return 'dynamic-fee';
  }
  if (parsed.accessList && parsed.accessList.length > 0) {
    return 'access-list';
  }
  return 'legacy';
}

function parseAddress(
  environmentId: EnvironmentId,
  value: string,
  label: string,
  required: boolean,
): ParseResult<string | undefined> {
  const trimmed = value.trim();

  if (!trimmed) {
    return required ? failure(`${label} is required.`) : success(undefined);
  }

  const environment = getEnvironment(environmentId);

  if (environment.addressKind === 'hex') {
    if (!isAddress(trimmed)) {
      return failure(`${label} must be a valid 0x address.`);
    }
    return success(getAddress(trimmed));
  }

  if (
    !isCoreAddress(trimmed) ||
    !trimmed.toLowerCase().startsWith(environment.coreAddressPrefix ?? '')
  ) {
    return failure(`${label} must be a valid Conflux Mainnet address.`);
  }

  return success(getCoreAddress(trimmed).toLowerCase());
}

function parseValue(
  environmentId: EnvironmentId,
  value: string,
): ParseResult<string | undefined> {
  const trimmed = value.trim();
  if (!trimmed) return success(undefined);

  try {
    const parsed =
      environmentId === 'ethereum-mainnet'
        ? parseEther(trimmed)
        : parseCFX(trimmed);
    if (parsed < 0n) return failure('Value must not be negative.');
    return success(toHex(parsed));
  } catch {
    return failure(`Value must be a valid ${getEnvironment(environmentId).nativeSymbol} amount.`);
  }
}

function parseData(value: string): ParseResult<string | undefined> {
  const trimmed = value.trim();
  if (!trimmed) return success(undefined);
  if (!DATA_PATTERN.test(trimmed)) {
    return failure('Data must be 0x-prefixed, byte-aligned hex.');
  }
  return success(trimmed.toLowerCase());
}

function parseFee(
  environmentId: EnvironmentId,
  value: string,
  label: string,
): ParseResult<string | undefined> {
  const trimmed = value.trim();
  if (!trimmed) return success(undefined);

  try {
    const parsed =
      environmentId === 'ethereum-mainnet'
        ? parseGwei(trimmed)
        : parseGDrip(trimmed);
    if (parsed < 0n) return failure(`${label} must not be negative.`);
    return success(toHex(parsed));
  } catch {
    return failure(`${label} must be a valid ${getEnvironment(environmentId).feeUnit} amount.`);
  }
}

function parseRequiredQuantity(
  value: string,
  label: string,
): ParseResult<string> {
  const result = parseOptionalQuantity(value, label);
  if (!result.ok) return result;
  return result.value
    ? success(result.value)
    : failure(`${label} is required when Number is selected.`);
}

function parseRequiredBlockHash(value: string): ParseResult<string> {
  const trimmed = value.trim();
  if (!trimmed) return failure('Block hash is required when Hash is selected.');
  if (!BLOCK_HASH_PATTERN.test(trimmed)) {
    return failure('Block hash must be a 32-byte 0x-prefixed value.');
  }
  return success(trimmed.toLowerCase());
}

function parseOptionalBlockReference(
  value: string,
): ParseResult<string | undefined> {
  const trimmed = value.trim();
  if (!trimmed) return success(undefined);
  if (BLOCK_HASH_PATTERN.test(trimmed)) {
    return success(trimmed.toLowerCase());
  }
  return parseOptionalQuantity(trimmed, 'Block number');
}

function parseOptionalQuantity(
  value: string,
  label: string,
): ParseResult<string | undefined> {
  const trimmed = value.trim();
  if (!trimmed) return success(undefined);

  try {
    const parsed = BigInt(trimmed);
    if (parsed < 0n) return failure(`${label} must not be negative.`);
    return success(toHex(parsed));
  } catch {
    return failure(`${label} must be a valid integer.`);
  }
}

function parseAccessList(
  environmentId: EnvironmentId,
  value: string,
): ParseResult<RpcAccessListItem[] | undefined> {
  const trimmed = value.trim();
  if (!trimmed || trimmed === '[]') return success(undefined);

  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (!Array.isArray(parsed)) {
      return failure('Access list must be a JSON array.');
    }

    const items: RpcAccessListItem[] = [];
    for (const [index, item] of parsed.entries()) {
      if (!item || typeof item !== 'object') {
        return failure(`Access list entry ${index + 1} must be an object.`);
      }

      const candidate = item as { address?: unknown; storageKeys?: unknown };
      if (typeof candidate.address !== 'string') {
        return failure(`Access list entry ${index + 1} has no valid address.`);
      }
      const address = parseAddress(
        environmentId,
        candidate.address,
        `Access list entry ${index + 1} address`,
        true,
      );
      if (!address.ok || !address.value) {
        return failure(
          address.ok
            ? `Access list entry ${index + 1} has no valid address.`
            : address.issue,
        );
      }

      if (!Array.isArray(candidate.storageKeys)) {
        return failure(
          `Access list entry ${index + 1} must include storageKeys.`,
        );
      }
      const storageKeys: string[] = [];
      for (const storageKey of candidate.storageKeys) {
        if (
          typeof storageKey !== 'string' ||
          !STORAGE_KEY_PATTERN.test(storageKey)
        ) {
          return failure(
            `Access list entry ${index + 1} contains an invalid storage key.`,
          );
        }
        storageKeys.push(storageKey.toLowerCase());
      }

      items.push({ address: address.value, storageKeys });
    }

    return success(items);
  } catch {
    return failure('Access list must be valid JSON.');
  }
}

function contextNumberLabel(environmentId: EnvironmentId) {
  return getEnvironment(environmentId).contextKind === 'block'
    ? 'Block number'
    : 'Epoch number';
}

function contextValueLabel(
  mode: ContextMode,
  environmentId: EnvironmentId,
) {
  return mode === 'hash' ? 'Block hash' : contextNumberLabel(environmentId);
}

function readParsed<TKey extends keyof SimulationFormValues, TValue>(
  issues: Partial<Record<keyof SimulationFormValues, string>>,
  field: TKey,
  result: ParseResult<TValue>,
): TValue | undefined {
  if (!result.ok) {
    issues[field] = result.issue;
    return undefined;
  }
  return result.value;
}

function success<T>(value: T): ParseSuccess<T> {
  return { ok: true, value };
}

function failure(issue: string): ParseFailure {
  return { ok: false, issue };
}

function assertNever(value: never): never {
  throw new Error(`Unhandled form field: ${String(value)}`);
}
