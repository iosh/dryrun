import type { EnvironmentId } from './environment.ts';
import type { RpcEnvelope, RpcSimulationResponse } from './rpc.ts';

type Check = (value: unknown, path: string) => void;

export function parseRpcEnvelope(value: unknown, requestId?: number): RpcEnvelope {
  const envelope = object(value, 'response');
  expect(envelope.jsonrpc === '2.0', 'response.jsonrpc');
  expect(
    envelope.id === null || typeof envelope.id === 'string' ||
      (typeof envelope.id === 'number' && Number.isFinite(envelope.id)),
    'response.id',
  );
  if (requestId !== undefined) expect(envelope.id === requestId, 'response.id');
  expect(('result' in envelope) !== ('error' in envelope), 'response.result/error');
  if ('error' in envelope) {
    const error = fields(envelope.error, 'response.error', { message: string });
    expect(typeof error.code === 'number' && Number.isInteger(error.code), 'response.error.code');
  }
  return value as RpcEnvelope;
}

// Validate the values consumed by the UI at both the network and history boundaries.
// Upstream transaction extensions remain intact for the raw transaction display.
export function parseSimulationResponse(
  value: unknown,
  environmentId: EnvironmentId,
): RpcSimulationResponse {
  const response = object(value, 'result');
  const core = environmentId === 'conflux-core-mainnet';
  fields(response.transaction, 'result.transaction', {
    from: address,
    to: optional(nullable(address)),
    chainId: quantity,
  });
  if (core) {
    expect(!('block' in response), 'result.block');
    fields(response.epoch, 'result.epoch', { number: quantity, pivotHash: hash });
    expect(response.approximate === true, 'result.approximate');
    array(string)(response.limitations, 'result.limitations');
  } else {
    expect(!('epoch' in response), 'result.epoch');
    fields(response.block, 'result.block', { number: quantity, hash });
  }

  const path = 'result.outcome';
  const outcome = object(response.outcome, path);
  if (outcome.status === 'rejected') {
    fields(outcome, path, { reason: string, message: string });
  } else {
    quantity(outcome.gasUsed, `${path}.gasUsed`);
    if (environmentId !== 'ethereum-mainnet') {
      quantity(outcome.gasCharged, `${path}.gasCharged`);
    } else {
      optional(quantity)(outcome.gasCharged, `${path}.gasCharged`);
    }
    const fee = fields(outcome.fee, `${path}.fee`, {
      gasPrice: quantity, baseFee: quantity, amount: quantity,
      blobGasPrice: optional(quantity),
    });
    if (core) {
      const payer = object(fee.payer, `${path}.fee.payer`);
      if (payer.type === 'sender') address(payer.address, `${path}.fee.payer.address`);
      else if (payer.type === 'sponsor') address(payer.contract, `${path}.fee.payer.contract`);
      else expect(false, `${path}.fee.payer.type`);
    } else {
      expect(fee.payer === undefined, `${path}.fee.payer`);
    }
    switch (outcome.status) {
      case 'success':
        bytes(outcome.output, `${path}.output`);
        checkChanges(outcome.changes, `${path}.changes`, core);
        break;
      case 'reverted':
        fields(outcome, path, { output: bytes, reason: optional(string) });
        break;
      case 'halted':
        string(outcome.reason, `${path}.reason`);
        break;
      default:
        expect(false, `${path}.status`);
    }
  }
  return value as RpcSimulationResponse;
}

function checkChanges(value: unknown, path: string, core: boolean) {
  const changes = object(value, path);
  if ('error' in changes) {
    expect(Object.keys(changes).length === 1, path);
    fields(changes.error, `${path}.error`, { code: string, message: string });
    return;
  }
  fields(changes, path, {
    balances: array((value, path) => {
      fields(value, path, { holder: address, asset: checkAsset, before: quantity, after: quantity });
    }),
    approvals: array(checkApproval),
    delegations: array((value, path) => {
      fields(value, path, { account: address, before: nullable(address), after: nullable(address) });
    }),
    contracts: array((value, path) => {
      fields(value, path, {
        address, called: boolean, created: boolean, destroyed: boolean, storageModified: boolean,
      });
    }),
    tokenReadFailures: array((value, path) => {
      fields(value, path, { token: address, function: string });
    }),
  });
  const tokens = object(changes.tokens, `${path}.tokens`);
  for (const [token, value] of Object.entries(tokens)) {
    address(token, `${path}.tokens key`);
    const metadata = fields(value, `${path}.tokens.${token}`, {
      name: optional(string), symbol: optional(string),
    });
    if (metadata.decimals !== undefined) {
      expect(
        typeof metadata.decimals === 'number' && Number.isInteger(metadata.decimals) &&
          metadata.decimals >= 0 && metadata.decimals <= 255,
        `${path}.tokens.${token}.decimals`,
      );
    }
  }
  if (core) array(checkProtocolChange)(changes.protocol, `${path}.protocol`);
  else expect(changes.protocol === undefined, `${path}.protocol`);
}

function checkAsset(value: unknown, path: string) {
  const asset = object(value, path);
  switch (asset.type) {
    case 'native':
      return;
    case 'erc20':
      address(asset.token, `${path}.token`);
      return;
    case 'erc721':
    case 'erc1155':
      fields(asset, path, { token: address, id: quantity });
      return;
    default:
      expect(false, `${path}.type`);
  }
}

function checkApproval(value: unknown, path: string) {
  const approval = fields(value, path, { token: address });
  switch (approval.type) {
    case 'erc20':
      fields(approval, path, { owner: address, spender: address, before: quantity, after: quantity });
      return;
    case 'erc721':
      fields(approval, path, { id: quantity, before: nullable(address), after: nullable(address) });
      return;
    case 'operator':
      fields(approval, path, { owner: address, operator: address, before: boolean, after: boolean });
      return;
    default:
      expect(false, `${path}.type`);
  }
}

function checkProtocolChange(value: unknown, path: string) {
  const change = object(value, path);
  let state: Check;
  switch (change.type) {
    case 'stakingBalance':
    case 'accumulatedInterestReturn':
    case 'collateralForStorage':
      address(change.account, `${path}.account`);
      state = quantity;
      break;
    case 'depositList':
      address(change.account, `${path}.account`);
      state = array((value, path) => {
        fields(value, path, { amount: quantity, depositTime: quantity, accumulatedInterestRate: quantity });
      });
      break;
    case 'voteStakeList':
      address(change.account, `${path}.account`);
      state = array((value, path) => {
        fields(value, path, { amount: quantity, unlockBlockNumber: quantity });
      });
      break;
    case 'posIdentifier':
      address(change.account, `${path}.account`);
      state = nullable(hash);
      break;
    case 'posStake':
      hash(change.identifier, `${path}.identifier`);
      state = (value, path) => {
        fields(value, path, { account: nullable(address), registered: quantity, unlocked: quantity });
      };
      break;
    case 'governanceVotes':
      address(change.voter, `${path}.voter`);
      state = array((value, path) => {
        const vote = fields(value, path, { index: quantity, votes: array(quantity) });
        expect((vote.votes as unknown[]).length === 3, `${path}.votes`);
      });
      break;
    case 'admin':
      address(change.contract, `${path}.contract`);
      state = nullable(address);
      break;
    case 'gasSponsor':
      address(change.contract, `${path}.contract`);
      state = (value, path) => {
        fields(value, path, { sponsor: nullable(address), balance: quantity, gasBound: quantity });
      };
      break;
    case 'storageSponsor':
      address(change.contract, `${path}.contract`);
      state = (value, path) => {
        fields(value, path, {
          sponsor: nullable(address), balance: quantity,
          storagePoints: nullable((value, path) => {
            fields(value, path, { unused: quantity, used: quantity });
          }),
        });
      };
      break;
    case 'sponsorWhitelist':
      fields(change, path, { contract: address, user: nullable(address) });
      state = boolean;
      break;
    default:
      expect(false, `${path}.type`);
      return;
  }
  state(change.before, `${path}.before`);
  state(change.after, `${path}.after`);
}

function expect(condition: boolean, path: string): asserts condition {
  if (!condition) throw new Error(`The service response has an invalid or missing ${path}.`);
}

function object(value: unknown, path: string): Record<string, unknown> {
  expect(value !== null && typeof value === 'object' && !Array.isArray(value), path);
  return value as Record<string, unknown>;
}

function fields(value: unknown, path: string, checks: Record<string, Check>) {
  const result = object(value, path);
  for (const [key, check] of Object.entries(checks)) check(result[key], `${path}.${key}`);
  return result;
}

function array(check: Check): Check {
  return (value, path) => {
    expect(Array.isArray(value), path);
    value.forEach((item, index) => check(item, `${path}[${index}]`));
  };
}

function optional(check: Check): Check {
  return (value, path) => { if (value !== undefined) check(value, path); };
}

function nullable(check: Check): Check {
  return (value, path) => { if (value !== null) check(value, path); };
}

function string(value: unknown, path: string) {
  expect(typeof value === 'string', path);
}

function boolean(value: unknown, path: string) {
  expect(typeof value === 'boolean', path);
}

function quantity(value: unknown, path: string) {
  expect(typeof value === 'string' && /^0x(?:0|[1-9a-f][0-9a-f]*)$/i.test(value), path);
}

function bytes(value: unknown, path: string) {
  expect(typeof value === 'string' && /^0x(?:[0-9a-f]{2})*$/i.test(value), path);
}

function hash(value: unknown, path: string) {
  expect(typeof value === 'string' && /^0x[0-9a-f]{64}$/i.test(value), path);
}

function address(value: unknown, path: string) {
  expect(
    typeof value === 'string' &&
      /^(?:0x[0-9a-f]{40}|(?:cfx|cfxtest|net\d+):(?:type\.[a-z]+:)?[a-z0-9]{42})$/i.test(value),
    path,
  );
}
