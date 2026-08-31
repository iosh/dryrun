import type { EnvironmentDefinition } from '../../environment.ts';
import type { AssetFlowItemViewModel } from '../../flowView.ts';
import type {
  SimulationChange,
  SimulationRecord,
} from '../../types.ts';

export interface AddressHighlightController {
  activeAddress: string | null;
  clearPinnedAddress: () => void;
  onAddressEnter: (address: string) => void;
  onAddressLeave: () => void;
  onAddressToggle: (address: string) => void;
  pinnedAddress: string | null;
}

export interface ExecutionAnchor {
  hash: string;
  label: 'Block' | 'Epoch';
  number: string;
}

export interface SequencedAssetFlowItemViewModel
  extends AssetFlowItemViewModel {
  changeIndex: number;
  flowIndex: number;
}

export type FlowLaneViewModel =
  | {
      address: string;
      alias: string;
      context?: string;
      key: string;
      kind: 'address';
    }
  | {
      alias: 'Burn' | 'Mint';
      key: string;
      kind: 'terminal';
    };

export interface SenderImpactItem {
  label: 'Fee' | 'Received' | 'Sent';
  tone: 'negative' | 'neutral' | 'positive';
  value: string;
}

export interface SimulationResultViewModel {
  anchor: ExecutionAnchor;
  changes: readonly SimulationChange[];
  changesError: string | null;
  environment: EnvironmentDefinition;
  flowItems: readonly SequencedAssetFlowItemViewModel[];
  lanes: readonly FlowLaneViewModel[];
  senderImpacts: readonly SenderImpactItem[];
  stateEffects: readonly SimulationChange[];
}

export type SimulationExecution =
  SimulationRecord['response']['execution'];

export type FlowSegment = 'full' | 'left-half' | 'right-half' | null;
