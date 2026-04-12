import {
  formatEther,
  formatGwei,
  isAddress,
  isHex,
  parseEther,
  parseGwei,
  toHex,
  type Address,
  type Hex,
} from 'viem';

import { formatJson } from '../lib/formatting.ts';
import { normalizeRpcAddress } from './client.ts';
import type {
  DryrunAccessListItem,
  DryrunBlockRef,
  DryrunSimulateTransactionRequest,
} from './rpc.ts';
import type { SimulationFormValues, TxTypeOption } from './types.ts';

const DEFAULT_GAS_LIMIT = 300_000n;

const txTypeToHex: Record<Exclude<TxTypeOption, 'auto'>, Hex> = {
  'access-list': '0x1',
  'dynamic-fee': '0x2',
  legacy: '0x0',
};

type SimulationFieldIssueMap = Partial<Record<keyof SimulationFormValues, string>>;

interface ParseSuccess<TValue> {
  ok: true;
  value: TValue;
}

interface ParseFailure {
  ok: false;
  issue: string;
}

type ParseResult<TValue> = ParseSuccess<TValue> | ParseFailure;

export interface ParsedSimulationValues {
  accessList?: DryrunAccessListItem[];
  block?: DryrunBlockRef;
  data?: Hex;
  from?: Address;
  gas?: Hex;
  gasPrice?: Hex;
  maxFeePerGas?: Hex;
  maxPriorityFeePerGas?: Hex;
  nonce?: Hex;
  to?: Address;
  value?: Hex;
}

export interface ParsedSimulationForm {
  fieldIssues: SimulationFieldIssueMap;
  formIssues: readonly string[];
  values: ParsedSimulationValues;
}

// Raw form strings -> validated RPC-ready primitives.
export function parseSimulationFormValues(
  formValues: SimulationFormValues,
): ParsedSimulationForm {
  const fieldIssues: SimulationFieldIssueMap = {};
  const values: ParsedSimulationValues = {};

  values.from = readParsedValue(
    fieldIssues,
    'from',
    parseRequiredAddress(formValues.from, 'From address'),
  );
  values.to = readParsedValue(
    fieldIssues,
    'to',
    parseOptionalAddress(formValues.to, 'To address'),
  );
  values.value = readParsedValue(
    fieldIssues,
    'valueEth',
    parseOptionalEtherValue(formValues.valueEth),
  );
  values.gas = readParsedValue(
    fieldIssues,
    'gasLimit',
    parseGasLimit(formValues.gasLimit),
  );
  values.data = readParsedValue(
    fieldIssues,
    'calldata',
    parseOptionalHex(formValues.calldata, 'Calldata', true),
  );
  values.block = readParsedValue(
    fieldIssues,
    'executionBlock',
    parseBlockRef(formValues.executionBlock),
  );
  values.nonce = readParsedValue(
    fieldIssues,
    'nonce',
    parseOptionalIntegerQuantity(formValues.nonce, 'Nonce'),
  );
  values.gasPrice = readParsedValue(
    fieldIssues,
    'gasPriceGwei',
    parseOptionalGweiQuantity(formValues.gasPriceGwei, 'Gas price'),
  );
  values.maxFeePerGas = readParsedValue(
    fieldIssues,
    'maxFeePerGasGwei',
    parseOptionalGweiQuantity(formValues.maxFeePerGasGwei, 'Max fee per gas'),
  );
  values.maxPriorityFeePerGas = readParsedValue(
    fieldIssues,
    'maxPriorityFeePerGasGwei',
    parseOptionalGweiQuantity(
      formValues.maxPriorityFeePerGasGwei,
      'Max priority fee per gas',
    ),
  );
  values.accessList = readParsedValue(
    fieldIssues,
    'accessListJson',
    parseAccessList(formValues.accessListJson),
  );

  return {
    fieldIssues,
    formIssues: validateFormRelationships(formValues.txType, values),
    values,
  };
}

export function validateSimulationField<TKey extends keyof SimulationFormValues>(
  field: TKey,
  value: SimulationFormValues[TKey],
) {
  switch (field) {
    case 'from':
      return toIssue(parseRequiredAddress(value, 'From address'));
    case 'to':
      return toIssue(parseOptionalAddress(value, 'To address'));
    case 'valueEth':
      return toIssue(parseOptionalEtherValue(value));
    case 'gasLimit':
      return toIssue(parseGasLimit(value));
    case 'calldata':
      return toIssue(parseOptionalHex(value, 'Calldata', true));
    case 'executionBlock':
      return toIssue(parseBlockRef(value));
    case 'nonce':
      return toIssue(parseOptionalIntegerQuantity(value, 'Nonce'));
    case 'gasPriceGwei':
      return toIssue(parseOptionalGweiQuantity(value, 'Gas price'));
    case 'maxFeePerGasGwei':
      return toIssue(parseOptionalGweiQuantity(value, 'Max fee per gas'));
    case 'maxPriorityFeePerGasGwei':
      return toIssue(
        parseOptionalGweiQuantity(value, 'Max priority fee per gas'),
      );
    case 'accessListJson':
      return toIssue(parseAccessList(value));
    case 'txType':
      return undefined;
    default:
      return assertNever(field);
  }
}

// Validated form values -> frontend RPC contract payload.
export function serializeSimulationRequest(
  formValues: SimulationFormValues,
): DryrunSimulateTransactionRequest | undefined {
  const parsed = parseSimulationFormValues(formValues);

  if (
    Object.keys(parsed.fieldIssues).length > 0 ||
    parsed.formIssues.length > 0 ||
    !parsed.values.from ||
    !parsed.values.gas ||
    !parsed.values.block
  ) {
    return undefined;
  }

  const {
    accessList,
    block,
    data,
    from,
    gas,
    gasPrice,
    maxFeePerGas,
    maxPriorityFeePerGas,
    nonce,
    to,
    value,
  } = parsed.values;

  return {
    block,
    transaction: {
      ...(accessList && accessList.length > 0 ? { accessList } : {}),
      ...(data ? { data } : {}),
      ...(gasPrice ? { gasPrice } : {}),
      ...(maxFeePerGas ? { maxFeePerGas } : {}),
      ...(maxPriorityFeePerGas ? { maxPriorityFeePerGas } : {}),
      ...(nonce ? { nonce } : {}),
      ...(to ? { to } : {}),
      ...(value ? { value } : {}),
      ...(formValues.txType !== 'auto'
        ? { type: txTypeToHex[formValues.txType] }
        : {}),
      from,
      gas,
    },
  };
}

// Stored RPC payload -> editable form strings.
export function hydrateSimulationFormValues(
  request: DryrunSimulateTransactionRequest,
): SimulationFormValues {
  const { block, transaction } = request;

  return {
    accessListJson: JSON.stringify(transaction.accessList ?? [], null, 2),
    calldata: transaction.data ?? '0x',
    executionBlock: formatBlockRef(block),
    from: transaction.from,
    gasLimit: formatQuantityInput(coerceHexQuantity(transaction.gas)),
    gasPriceGwei: formatGweiInput(coerceHexQuantity(transaction.gasPrice)),
    maxFeePerGasGwei: formatGweiInput(coerceHexQuantity(transaction.maxFeePerGas)),
    maxPriorityFeePerGasGwei: formatGweiInput(
      coerceHexQuantity(transaction.maxPriorityFeePerGas),
    ),
    nonce: formatQuantityInput(coerceHexQuantity(transaction.nonce)),
    to: transaction.to ?? '',
    txType: formatTxType(coerceHexQuantity(transaction.type)),
    valueEth: formatEtherInput(coerceHexQuantity(transaction.value)),
  };
}

// Preview uses the same serialization path as submit.
export function formatSimulationRequestPreviewJson(
  formValues: SimulationFormValues,
) {
  const request = serializeSimulationRequest(formValues);

  return formatJson({
    id: 1,
    jsonrpc: '2.0',
    method: 'dryrun_evm_simulateTransaction',
    params: request ?? null,
  });
}

function validateFormRelationships(
  txType: TxTypeOption,
  values: ParsedSimulationValues,
) {
  const issues: string[] = [];

  if (values.gasPrice && (values.maxFeePerGas || values.maxPriorityFeePerGas)) {
    issues.push(
      'Gas price cannot be mixed with EIP-1559 fee fields in the same request.',
    );
  }

  if (
    txType === 'legacy' &&
    (values.maxFeePerGas || values.maxPriorityFeePerGas)
  ) {
    issues.push('Legacy transactions cannot include EIP-1559 fee fields.');
  }

  if (txType === 'dynamic-fee' && values.gasPrice) {
    issues.push('Dynamic-fee transactions cannot include a gas price.');
  }

  if (values.maxFeePerGas && values.maxPriorityFeePerGas) {
    try {
      if (BigInt(values.maxPriorityFeePerGas) > BigInt(values.maxFeePerGas)) {
        issues.push('Max priority fee per gas cannot exceed max fee per gas.');
      }
    } catch {
      issues.push('Invalid EIP-1559 fee values.');
    }
  }

  return issues;
}

function readParsedValue<TKey extends keyof SimulationFormValues, TValue>(
  fieldIssues: SimulationFieldIssueMap,
  field: TKey,
  result: ParseResult<TValue>,
) {
  if (isParseFailure(result)) {
    fieldIssues[field] = result.issue;
    return undefined;
  }

  return result.value;
}

function toIssue<TValue>(result: ParseResult<TValue>) {
  return isParseFailure(result) ? result.issue : undefined;
}

function parseRequiredAddress(
  value: string,
  field: string,
): ParseResult<Address> {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return failure(`${field} is required.`);
  }

  const result = parseOptionalAddress(trimmed, field);

  if (isParseFailure(result)) {
    return result;
  }

  if (!result.value) {
    return failure(`${field} is required.`);
  }

  return success(result.value);
}

function parseOptionalAddress(
  value: string,
  field: string,
): ParseResult<Address | undefined> {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return success(undefined);
  }

  if (!isAddress(trimmed)) {
    return failure(`${field} must be a valid Ethereum address.`);
  }

  return success(normalizeRpcAddress(trimmed) as Address);
}

function parseOptionalHex(
  value: string,
  field: string,
  allowZero = false,
): ParseResult<Hex | undefined> {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return success(undefined);
  }

  if (!isHex(trimmed)) {
    return failure(`${field} must be a 0x-prefixed hex value.`);
  }

  if (!allowZero && trimmed === '0x') {
    return failure(`${field} cannot be empty hex.`);
  }

  return success(trimmed as Hex);
}

function parseGasLimit(value: string): ParseResult<Hex> {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return success(toHex(DEFAULT_GAS_LIMIT));
  }

  return parsePositiveQuantity(trimmed, 'Gas limit');
}

function parseOptionalIntegerQuantity(
  value: string,
  field: string,
): ParseResult<Hex | undefined> {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return success(undefined);
  }

  return parsePositiveQuantity(trimmed, field);
}

function parseOptionalEtherValue(value: string): ParseResult<Hex | undefined> {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return success(undefined);
  }

  try {
    return success(toHex(parseEther(trimmed)));
  } catch {
    return failure('Value must be a valid ETH amount.');
  }
}

function parseOptionalGweiQuantity(
  value: string,
  field: string,
): ParseResult<Hex | undefined> {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return success(undefined);
  }

  try {
    return success(toHex(parseGwei(trimmed)));
  } catch {
    return failure(`${field} must be a valid Gwei amount.`);
  }
}

function parsePositiveQuantity(value: string, field: string): ParseResult<Hex> {
  try {
    const normalized = BigInt(value);

    if (normalized < 0n) {
      return failure(`${field} must not be negative.`);
    }

    return success(toHex(normalized));
  } catch {
    return failure(`${field} must be a valid integer.`);
  }
}

function parseBlockRef(value: string): ParseResult<DryrunBlockRef> {
  const trimmed = value.trim();

  if (trimmed.length === 0 || trimmed === 'latest') {
    return success('latest');
  }

  if (trimmed.startsWith('0x')) {
    if (!isHex(trimmed)) {
      return failure('Execution block must be `latest` or a hex block number.');
    }

    if (trimmed.length === 66) {
      return failure('Block hash selectors are reserved and not supported yet.');
    }

    return success(trimmed as Hex);
  }

  try {
    return success(toHex(BigInt(trimmed)));
  } catch {
    return failure('Execution block must be `latest` or a block number.');
  }
}

function parseAccessList(
  value: string,
): ParseResult<DryrunAccessListItem[] | undefined> {
  const trimmed = value.trim();

  if (trimmed.length === 0 || trimmed === '[]') {
    return success(undefined);
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown;

    if (!Array.isArray(parsed)) {
      return failure('Access list JSON must be an array.');
    }

    const accessList: DryrunAccessListItem[] = [];

    for (const [index, entry] of parsed.entries()) {
      const result = parseAccessListItem(entry, index);

      if (isParseFailure(result)) {
        return failure(result.issue);
      }

      accessList.push(result.value);
    }

    return success(accessList);
  } catch {
    return failure('Access list JSON must be valid JSON.');
  }
}

function parseAccessListItem(
  entry: unknown,
  index: number,
): ParseResult<DryrunAccessListItem> {
  if (!entry || typeof entry !== 'object') {
    return failure(`Access list entry ${index + 1} must be an object.`);
  }

  const { address, storageKeys } = entry as {
    address?: unknown;
    storageKeys?: unknown;
  };

  if (typeof address !== 'string' || !isAddress(address)) {
    return failure(`Access list entry ${index + 1} has an invalid address.`);
  }

  if (!Array.isArray(storageKeys)) {
    return failure(
      `Access list entry ${index + 1} must provide a storageKeys array.`,
    );
  }

  const normalizedStorageKeys = storageKeys
    .filter((value): value is string => typeof value === 'string')
    .map((value) => value.trim());

  if (
    normalizedStorageKeys.length !== storageKeys.length ||
    normalizedStorageKeys.some((value) => !isHex(value))
  ) {
    return failure(
      `Access list entry ${index + 1} contains an invalid storage key.`,
    );
  }

  return success({
    address: normalizeRpcAddress(address) as Address,
    storageKeys: normalizedStorageKeys as Hex[],
  });
}

function formatQuantityInput(value?: Hex) {
  if (!value) {
    return '';
  }

  try {
    return BigInt(value).toString();
  } catch {
    return value;
  }
}

function coerceHexQuantity(value?: string): Hex | undefined {
  if (!value || !isHex(value)) {
    return undefined;
  }

  return value;
}

function formatEtherInput(value?: Hex) {
  if (!value) {
    return '';
  }

  try {
    return formatEther(BigInt(value));
  } catch {
    return value;
  }
}

function formatGweiInput(value?: Hex) {
  if (!value) {
    return '';
  }

  try {
    return formatGwei(BigInt(value));
  } catch {
    return value;
  }
}

function formatBlockRef(block?: DryrunBlockRef) {
  if (!block || block === 'latest') {
    return 'latest';
  }

  if (typeof block === 'string') {
    try {
      return BigInt(block).toString();
    } catch {
      return block;
    }
  }

  return block.blockHash;
}

function formatTxType(value?: Hex): TxTypeOption {
  switch (value) {
    case '0x0':
      return 'legacy';
    case '0x1':
      return 'access-list';
    case '0x2':
      return 'dynamic-fee';
    default:
      return 'auto';
  }
}

function success<TValue>(value: TValue): ParseSuccess<TValue> {
  return {
    ok: true,
    value,
  };
}

function failure(issue: string): ParseFailure {
  return {
    issue,
    ok: false,
  };
}

function isParseFailure<TValue>(
  result: ParseResult<TValue>,
): result is ParseFailure {
  return result.ok === false;
}

function assertNever(value: never): never {
  throw new Error(`Unhandled simulation field: ${String(value)}`);
}
