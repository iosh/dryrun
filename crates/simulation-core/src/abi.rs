use std::collections::HashMap;

use alloy::{
    contract::Interface,
    dyn_abi::EventExt,
    hex,
    json_abi::{Event, JsonAbi},
    primitives::{Function, Selector},
    rlp::{Bytes, bytes},
    rpc::types::Log,
    sol_types::{SolValue, sol_data::FixedBytes},
};
use revm::primitives::B256;
use types::{CallTraceDecodedParam, DecodeLogInput};
pub struct AbiDecoder {
    abi: JsonAbi,
    interface: Interface,
    event_map: HashMap<B256, Event>,
}

impl AbiDecoder {
    pub fn new(abi: JsonAbi) -> Self {
        let event_map = abi.events().map(|e| (e.selector(), e.clone())).collect();

        Self {
            abi: abi.clone(),
            interface: Interface::new(abi),
            event_map,
        }
    }

    pub fn decode_log(&self, raw_log: &Log) -> Option<(String, bool, Vec<DecodeLogInput>)> {
        let log_data = raw_log.data();
        if log_data.topics().is_empty() {
            return None;
        }

        let signature_topic = log_data.topics()[0];

        if let Some(event) = self.event_map.get(&signature_topic) {
            if let Ok(decode_event) = event.decode_log(log_data) {
                let params = event
                    .inputs
                    .iter()
                    .zip(decode_event.indexed.iter().chain(decode_event.body.iter()))
                    .map(|(param, token)| {
                        let value = match token {
                            alloy::dyn_abi::DynSolValue::Bytes(bytes) => {
                                format!("0x{}", hex::encode(bytes))
                            }
                            alloy::dyn_abi::DynSolValue::FixedBytes(bytes, _) => {
                                format!("0x{}", hex::encode(bytes))
                            }

                            alloy::dyn_abi::DynSolValue::Address(addr) => format!("0x{:x}", addr),

                            alloy::dyn_abi::DynSolValue::Uint(num, _) => num.to_string(),
                            alloy::dyn_abi::DynSolValue::Int(num, _) => num.to_string(),

                            alloy::dyn_abi::DynSolValue::Bool(b) => b.to_string(),

                            alloy::dyn_abi::DynSolValue::String(s) => s.clone(),

                            alloy::dyn_abi::DynSolValue::Array(arr) => format!(
                                "[{}]",
                                arr.iter()
                                    .map(|v| format!("{:?}", v))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),

                            _ => format!("{:?}", token),
                        };

                        DecodeLogInput {
                            name: param.name.clone(),
                            sol_type: param.ty.clone(),
                            value,
                            indexed: param.indexed,
                        }
                    })
                    .collect();
                return Some((event.name.clone(), event.anonymous, params));
            }
        }

        None
    }
}
