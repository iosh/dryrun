use std::collections::BTreeMap;

use alloy_primitives::{B256, Bytes, U256};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::CoreAddress;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosDecision {
    pub block_hash: B256,
    pub height: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PosBlockNumber {
    Number(U256),
    Earliest,
    LatestCommitted,
    LatestVoted,
}

impl Serialize for PosBlockNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Number(value) => serializer.serialize_str(&format!("0x{value:x}")),
            Self::Earliest => serializer.serialize_str("earliest"),
            Self::LatestCommitted => serializer.serialize_str("latest_committed"),
            Self::LatestVoted => serializer.serialize_str("latest_voted"),
        }
    }
}

impl<'de> Deserialize<'de> for PosBlockNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "earliest" => Ok(Self::Earliest),
            "latest_committed" => Ok(Self::LatestCommitted),
            "latest_voted" => Ok(Self::LatestVoted),
            value if value.starts_with("0x") => U256::from_str_radix(&value[2..], 16)
                .map(Self::Number)
                .map_err(|error| serde::de::Error::custom(error.to_string())),
            _ => Err(serde::de::Error::custom("invalid PoS block number")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosSignature {
    pub account: B256,
    pub votes: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosBlockFull {
    pub hash: B256,
    pub height: U256,
    pub epoch: U256,
    pub round: U256,
    pub last_tx_number: U256,
    pub miner: Option<B256>,
    pub parent_hash: B256,
    pub timestamp: U256,
    pub pivot_decision: Option<PosDecision>,
    pub signatures: Vec<PosSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosStatus {
    pub latest_committed: U256,
    pub epoch: U256,
    pub pivot_decision: PosDecision,
    pub latest_voted: Option<U256>,
    pub latest_tx_number: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosAccount {
    pub address: B256,
    pub block_number: U256,
    pub status: PosNodeLockStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosNodeLockStatus {
    pub in_queue: Vec<PosVotePowerState>,
    pub locked: U256,
    pub out_queue: Vec<PosVotePowerState>,
    pub unlocked: U256,
    pub available_votes: U256,
    pub force_retired: Option<U256>,
    pub forfeited: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosVotePowerState {
    pub end_block_number: U256,
    pub power: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosNodeVotingPower {
    pub address: B256,
    pub voting_power: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosCommitteeState {
    pub current_committee: PosCommittee,
    pub elections: Vec<PosTermData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PosCommittee {
    pub epoch_number: U256,
    pub quorum_voting_power: U256,
    pub total_voting_power: U256,
    pub nodes: Vec<PosNodeVotingPower>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PosTermData {
    pub start_block_number: U256,
    pub is_finalized: bool,
    pub top_electing_nodes: Vec<PosNodeVotingPower>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosEpochState {
    pub epoch: U256,
    pub verifier: PosValidatorVerifier,
    pub vrf_seed: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosValidatorVerifier {
    pub address_to_validator_info: BTreeMap<B256, PosValidatorConsensusInfo>,
    pub quorum_voting_power: U256,
    pub total_voting_power: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosValidatorConsensusInfo {
    pub public_key: Bytes,
    pub vrf_public_key: Option<Bytes>,
    pub voting_power: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosLedgerInfoWithSignatures {
    pub ledger_info: PosLedgerInfo,
    pub signatures: BTreeMap<B256, Bytes>,
    pub next_epoch_validators: Option<BTreeMap<B256, Bytes>>,
    pub aggregated_signature: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosLedgerInfo {
    pub commit_info: PosBlockInfo,
    pub consensus_data_hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosBlockInfo {
    pub epoch: U256,
    pub round: U256,
    pub id: B256,
    pub executed_state_id: B256,
    pub version: U256,
    pub timestamp_usecs: U256,
    pub next_epoch_state: Option<PosEpochState>,
    pub pivot: Option<PosDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosReward {
    pub pos_address: B256,
    pub pow_address: CoreAddress,
    pub reward: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosEpochReward {
    pub pow_epoch_hash: B256,
    pub account_rewards: Vec<PosReward>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PosTransactionStatus {
    Executed,
    Failed,
    Discard,
    Unknown(String),
}

impl Serialize for PosTransactionStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::Executed => "Executed",
            Self::Failed => "Failed",
            Self::Discard => "Discard",
            Self::Unknown(value) => value,
        })
    }
}

impl<'de> Deserialize<'de> for PosTransactionStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "Executed" => Self::Executed,
            "Failed" => Self::Failed,
            "Discard" => Self::Discard,
            _ => Self::Unknown(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PosTransactionType {
    BlockMetadata,
    Election,
    Retire,
    Register,
    UpdateVotingPower,
    PivotDecision,
    Dispute,
    Other,
    Unknown(String),
}

impl Serialize for PosTransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::BlockMetadata => "BlockMetadata",
            Self::Election => "Election",
            Self::Retire => "Retire",
            Self::Register => "Register",
            Self::UpdateVotingPower => "UpdateVotingPower",
            Self::PivotDecision => "PivotDecision",
            Self::Dispute => "Dispute",
            Self::Other => "Other",
            Self::Unknown(value) => value,
        })
    }
}

impl<'de> Deserialize<'de> for PosTransactionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "BlockMetadata" => Self::BlockMetadata,
            "Election" => Self::Election,
            "Retire" => Self::Retire,
            "Register" => Self::Register,
            "UpdateVotingPower" => Self::UpdateVotingPower,
            "PivotDecision" => Self::PivotDecision,
            "Dispute" => Self::Dispute,
            "Other" => Self::Other,
            _ => Self::Unknown(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosRegisterPayload {
    pub vrf_public_key: String,
    pub public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosElectionPayload {
    pub public_key: String,
    pub target_term: U256,
    pub vrf_proof: String,
    pub vrf_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosUpdateVotingPowerPayload {
    pub address: B256,
    pub voting_power: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosPivotDecisionPayload {
    pub height: U256,
    pub block_hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosRetirePayload {
    pub address: B256,
    pub voting_power: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosConflictingVotes {
    pub conflict_vote_type: String,
    pub first: String,
    pub second: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosDisputePayload {
    pub address: B256,
    pub bls_public_key: String,
    pub vrf_public_key: String,
    pub conflicting_votes: PosConflictingVotes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum PosTransactionPayload {
    Register(PosRegisterPayload),
    Election(PosElectionPayload),
    UpdateVotingPower(PosUpdateVotingPowerPayload),
    PivotDecision(PosPivotDecisionPayload),
    Retire(PosRetirePayload),
    Dispute(PosDisputePayload),
    Unknown(serde_json::Value),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PosTransactionWire {
    hash: B256,
    from: B256,
    block_hash: Option<B256>,
    block_number: Option<U256>,
    timestamp: Option<U256>,
    number: U256,
    payload: Option<serde_json::Value>,
    status: Option<PosTransactionStatus>,
    #[serde(rename = "type")]
    transaction_type: PosTransactionType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PosTransaction {
    pub hash: B256,
    pub from: B256,
    pub block_hash: Option<B256>,
    pub block_number: Option<U256>,
    pub timestamp: Option<U256>,
    pub number: U256,
    pub payload: Option<PosTransactionPayload>,
    pub status: Option<PosTransactionStatus>,
    #[serde(rename = "type")]
    pub transaction_type: PosTransactionType,
}

impl<'de> Deserialize<'de> for PosTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PosTransactionWire::deserialize(deserializer)?;
        let payload = wire
            .payload
            .map(|value| decode_pos_transaction_payload(&wire.transaction_type, value))
            .transpose()?;
        Ok(Self {
            hash: wire.hash,
            from: wire.from,
            block_hash: wire.block_hash,
            block_number: wire.block_number,
            timestamp: wire.timestamp,
            number: wire.number,
            payload,
            status: wire.status,
            transaction_type: wire.transaction_type,
        })
    }
}

fn decode_pos_transaction_payload<E: serde::de::Error>(
    transaction_type: &PosTransactionType,
    value: serde_json::Value,
) -> Result<PosTransactionPayload, E> {
    match transaction_type {
        PosTransactionType::Register => serde_json::from_value::<PosRegisterPayload>(value)
            .map(PosTransactionPayload::Register)
            .map_err(|error| E::custom(error.to_string())),
        PosTransactionType::Election => serde_json::from_value::<PosElectionPayload>(value)
            .map(PosTransactionPayload::Election)
            .map_err(|error| E::custom(error.to_string())),
        PosTransactionType::UpdateVotingPower => {
            serde_json::from_value::<PosUpdateVotingPowerPayload>(value)
                .map(PosTransactionPayload::UpdateVotingPower)
                .map_err(|error| E::custom(error.to_string()))
        }
        PosTransactionType::PivotDecision => {
            serde_json::from_value::<PosPivotDecisionPayload>(value)
                .map(PosTransactionPayload::PivotDecision)
                .map_err(|error| E::custom(error.to_string()))
        }
        PosTransactionType::Retire => serde_json::from_value::<PosRetirePayload>(value)
            .map(PosTransactionPayload::Retire)
            .map_err(|error| E::custom(error.to_string())),
        PosTransactionType::Dispute => serde_json::from_value::<PosDisputePayload>(value)
            .map(PosTransactionPayload::Dispute)
            .map_err(|error| E::custom(error.to_string())),
        PosTransactionType::BlockMetadata
        | PosTransactionType::Other
        | PosTransactionType::Unknown(_) => Ok(PosTransactionPayload::Unknown(value)),
    }
}
