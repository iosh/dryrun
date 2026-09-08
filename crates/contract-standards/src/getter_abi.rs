use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{Error as AbiError, SolCall, sol};

sol! {
    interface IERC20State {
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function totalSupply() external view returns (uint256);
    }

    interface IERC721State {
        function ownerOf(uint256 tokenId) external view returns (address);
        function getApproved(uint256 tokenId) external view returns (address);
        function isApprovedForAll(address owner, address operator) external view returns (bool);
    }

    interface IERC1155State {
        function balanceOf(address account, uint256 id) external view returns (uint256);
    }
}

pub mod erc20 {
    use super::*;

    pub fn balance_of_call(account: Address) -> Bytes {
        IERC20State::balanceOfCall { account }.abi_encode().into()
    }

    pub fn decode_balance_of_output(output: &[u8]) -> Result<U256, AbiError> {
        IERC20State::balanceOfCall::abi_decode_returns_validate(output)
    }

    pub fn allowance_call(owner: Address, spender: Address) -> Bytes {
        IERC20State::allowanceCall { owner, spender }
            .abi_encode()
            .into()
    }

    pub fn decode_allowance_output(output: &[u8]) -> Result<U256, AbiError> {
        IERC20State::allowanceCall::abi_decode_returns_validate(output)
    }

    pub fn total_supply_call() -> Bytes {
        IERC20State::totalSupplyCall {}.abi_encode().into()
    }

    pub fn decode_total_supply_output(output: &[u8]) -> Result<U256, AbiError> {
        IERC20State::totalSupplyCall::abi_decode_returns_validate(output)
    }
}

pub mod erc721 {
    use super::*;

    pub fn owner_of_call(token_id: U256) -> Bytes {
        IERC721State::ownerOfCall { tokenId: token_id }
            .abi_encode()
            .into()
    }

    pub fn decode_owner_of_output(output: &[u8]) -> Result<Address, AbiError> {
        IERC721State::ownerOfCall::abi_decode_returns_validate(output)
    }

    pub fn get_approved_call(token_id: U256) -> Bytes {
        IERC721State::getApprovedCall { tokenId: token_id }
            .abi_encode()
            .into()
    }

    pub fn decode_get_approved_output(output: &[u8]) -> Result<Address, AbiError> {
        IERC721State::getApprovedCall::abi_decode_returns_validate(output)
    }

    pub fn is_approved_for_all_call(owner: Address, operator: Address) -> Bytes {
        IERC721State::isApprovedForAllCall { owner, operator }
            .abi_encode()
            .into()
    }

    pub fn decode_is_approved_for_all_output(output: &[u8]) -> Result<bool, AbiError> {
        IERC721State::isApprovedForAllCall::abi_decode_returns_validate(output)
    }
}

pub mod erc1155 {
    use super::*;

    pub fn balance_of_call(account: Address, token_id: U256) -> Bytes {
        IERC1155State::balanceOfCall {
            account,
            id: token_id,
        }
        .abi_encode()
        .into()
    }

    pub fn decode_balance_of_output(output: &[u8]) -> Result<U256, AbiError> {
        IERC1155State::balanceOfCall::abi_decode_returns_validate(output)
    }
}
