use alloy_sol_types::sol;

sol! {
    contract IERC165State {
        function supportsInterface(bytes4 interfaceId) external view returns (bool);
    }

    contract IERC20State {
        function balanceOf(address account) external view returns (uint256);
        function totalSupply() external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
    }

    contract IERC721State {
        function ownerOf(uint256 tokenId) external view returns (address);
        function getApproved(uint256 tokenId) external view returns (address);
    }

    contract IERC1155State {
        function balanceOf(address account, uint256 id) external view returns (uint256);
    }

    contract IOperatorApprovalState {
        function isApprovedForAll(address owner, address operator) external view returns (bool);
    }
}

pub use IERC20State::{
    allowanceCall as Erc20AllowanceCall, balanceOfCall as Erc20BalanceCall,
    totalSupplyCall as Erc20TotalSupplyCall,
};
pub use IERC165State::supportsInterfaceCall as SupportsInterfaceCall;
pub use IERC721State::{getApprovedCall as Erc721GetApprovedCall, ownerOfCall as Erc721OwnerCall};
pub use IERC1155State::balanceOfCall as Erc1155BalanceCall;
pub use IOperatorApprovalState::isApprovedForAllCall as OperatorApprovalCall;
