use std::collections::HashMap;

use alloy::{
    contract::Interface,
    dyn_abi::{DynSolValue, EventExt, JsonAbiExt},
    hex,
    json_abi::{Event, JsonAbi},
    primitives::Selector,
};
use revm::primitives::{B256, LogData};
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

    pub fn decode_input(&self, data: &[u8]) -> Option<(String, Vec<CallTraceDecodedParam>)> {
        if data.len() < 4 {
            return None;
        }

        let selector = Selector::from_slice(&data[..4]);
        let function = self.abi.functions().find(|f| f.selector() == selector)?;

        let decoded = function.abi_decode_input(&data[4..]).ok()?;

        let params = function
            .inputs
            .iter()
            .zip(decoded.iter())
            .map(|(param, token)| CallTraceDecodedParam {
                name: param.name.clone(),
                sol_type: param.ty.clone(),
                value: Self::format_sol_value(token),
            })
            .collect();

        Some((function.name.clone(), params))
    }

    pub fn decode_log(&self, raw_log: &LogData) -> Option<(String, bool, Vec<DecodeLogInput>)> {
        let topics = raw_log.topics();
        if topics.is_empty() {
            return None;
        }

        let signature_topic = topics[0];
        let event = self.event_map.get(&signature_topic)?;

        let decode_event = event.decode_log(raw_log).ok()?;

        let params = event
            .inputs
            .iter()
            .zip(decode_event.indexed.iter().chain(decode_event.body.iter()))
            .map(|(param, token)| DecodeLogInput {
                name: param.name.clone(),
                sol_type: param.ty.clone(),
                value: Self::format_sol_value(token),
                indexed: param.indexed,
            })
            .collect();

        Some((event.name.clone(), event.anonymous, params))
    }

    fn format_sol_value(value: &DynSolValue) -> String {
        match value {
            DynSolValue::Bytes(bytes) => format!("0x{}", hex::encode(bytes)),
            DynSolValue::FixedBytes(bytes, _) => format!("0x{}", hex::encode(bytes)),
            DynSolValue::Address(addr) => format!("0x{:x}", addr),
            DynSolValue::Uint(num, _) => format!("0x{:x}", num),
            DynSolValue::Int(num, _) => format!("0x{:x}", num),
            DynSolValue::Bool(b) => b.to_string(),
            DynSolValue::String(s) => s.clone(),
            DynSolValue::Array(arr) => {
                let elements: Vec<String> = arr.iter().map(Self::format_sol_value).collect();
                format!("[{}]", elements.join(", "))
            }
            DynSolValue::Tuple(tuple) => {
                let elements: Vec<String> = tuple.iter().map(Self::format_sol_value).collect();
                format!("({})", elements.join(", "))
            }
            _ => format!("{:?}", value),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use alloy::json_abi::JsonAbi;
    use revm::primitives::LogData;

    #[test]
    fn test_decode_input() {
        let abi_json = r#"[{
          "name": "transfer",
          "type": "function",
          "inputs": [
            {
              "name": "_to",
              "type": "address"
            },
            {
              "name": "_value",
              "type": "uint256"
            }
          ],
          "outputs": [],
          "payable": false,
          "constant": false,
          "stateMutability": "nonpayable"
        }]"#;

        let abi: JsonAbi = serde_json::from_str(abi_json).unwrap();

        let decoder = AbiDecoder::new(abi);

        let test_data = hex::decode("a9059cbb000000000000000000000000888888888888888888888888888888888888888800000000000000000000000000000000000000000000000000000000017d7840").unwrap();

        let result = decoder.decode_input(&test_data).unwrap();

        assert_eq!(result.0, "transfer");
        assert_eq!(result.1.len(), 2);
        assert_eq!(result.1[0].name, "_to");
        assert_eq!(result.1[0].sol_type, "address");
        assert_eq!(
            result.1[0].value,
            "0x8888888888888888888888888888888888888888"
        );

        assert_eq!(result.1[1].name, "_value");
        assert_eq!(result.1[1].sol_type, "uint256");
        assert_eq!(result.1[1].value, "0x17d7840");
    }

    #[test]
    fn test_decode_log() {
        let abi_json = r#"[{
          "name": "Transfer",
          "type": "event",
          "inputs": [
            {
              "name": "from",
              "type": "address",
              "indexed": true
            },
            {
              "name": "to",
              "type": "address",
              "indexed": true
            },
            {
              "name": "value",
              "type": "uint256"
            }
          ],
          "anonymous": false
        }]"#;

        let abi: JsonAbi = serde_json::from_str(abi_json).unwrap();
        let decoder = AbiDecoder::new(abi);

        let log_data = LogData::new(
            vec![
                B256::from_slice(
                    &hex::decode(
                        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                    )
                    .unwrap(),
                ),
                B256::from_slice(
                    &hex::decode(
                        "0x0000000000000000000000008888888888888888888888888888888888888888",
                    )
                    .unwrap(),
                ),
                B256::from_slice(
                    &hex::decode(
                        "0x000000000000000000000000b22499ac3b9fb4206d0eb620d1387c1d78a0d61d",
                    )
                    .unwrap(),
                ),
            ],
            hex::decode("0x00000000000000000000000000000000000000000000000000000000017d7840")
                .unwrap()
                .into(),
        )
        .unwrap();

        let result = decoder.decode_log(&log_data).unwrap();
        println!("{:?}", result);
        assert_eq!(result.0, "Transfer");
        assert!(!result.1);

        assert_eq!(result.2.len(), 3);
        assert_eq!(result.2[0].name, "from");
        assert_eq!(result.2[0].sol_type, "address");
        assert_eq!(
            result.2[0].value,
            "0x8888888888888888888888888888888888888888"
        );

        assert!(result.2[0].indexed);
        assert_eq!(result.2[1].name, "to");
        assert_eq!(result.2[1].sol_type, "address");
        assert_eq!(
            result.2[1].value,
            "0xb22499ac3b9fb4206d0eb620d1387c1d78a0d61d"
        );

        assert!(result.2[1].indexed);
        assert_eq!(result.2[2].name, "value");
        assert_eq!(result.2[2].sol_type, "uint256");
        assert_eq!(result.2[2].value, "0x17d7840");
        assert!(!result.2[2].indexed);
    }
}
