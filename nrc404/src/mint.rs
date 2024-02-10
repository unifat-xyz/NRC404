use near_sdk::require;
use crate::*;

#[near_bindgen]
impl Contract {

    #[payable]
    pub fn nft_wrap_by_owner(
        &mut self,
        metadata: TokenMetadata,
        receiver_id: AccountId,
        //we add an optional parameter for perpetual royalties
        perpetual_royalties: Option<HashMap<AccountId, u32>>,
    ) {
        require!(env::predecessor_account_id() == self.owner_id, "Illegal permissions");
        //measure the initial storage being used on the contract
        let initial_storage_usage = env::storage_usage();
        self.internal_mint(env::predecessor_account_id(), metadata, receiver_id, perpetual_royalties);

        //calculate the required storage which was the used - initial
        let required_storage_in_bytes = env::storage_usage() - initial_storage_usage;

        //refund any excess storage if the user attached too much. Panic if they didn't attach enough to cover the required.
        refund_deposit(required_storage_in_bytes);
    }

    #[payable]
    pub fn nft_wrap(
        &mut self,
        count: U128,
    ) {
        let ft_balance = self.internal_unwrap_balance_of(&env::predecessor_account_id());
        let metadata = self.metadata.get().unwrap();
        let decimal_int = 10u128.pow(metadata.decimals as u32);
        require!(ft_balance / decimal_int >= count.0, LESS_BALANCE);
        self.internal_wrap_ft_to_nft_with_count(&env::predecessor_account_id(), &metadata, count.0);
    }
}
