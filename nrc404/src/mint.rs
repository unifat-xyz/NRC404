use near_sdk::require;
use crate::*;

#[near_bindgen]
impl Contract {

    #[payable]
    pub fn nft_wrap_by_operator(
        &mut self,
        receiver_id: AccountId,
        //we add an optional parameter for perpetual royalties
        perpetual_royalties: Option<HashMap<AccountId, u32>>,
        count: u8,
    ) {
        require!(env::predecessor_account_id() == self.operator || env::predecessor_account_id() == self.owner_id, "Illegal permissions");
        //measure the initial storage being used on the contract
        let initial_storage_usage = env::storage_usage();
        let metadata = self.metadata.get().unwrap();
        for _ in 0..count {
            let level = self.internal_get_new_level(&env::predecessor_account_id(), &metadata, true);
            let metadata = TokenMetadata {
                level
            };
            self.internal_mint(env::predecessor_account_id(), metadata, receiver_id.clone(), perpetual_royalties.clone());
        }
        //calculate the required storage which was the used - initial
        let required_storage_in_bytes = env::storage_usage() - initial_storage_usage;

        //refund any excess storage if the user attached too much. Panic if they didn't attach enough to cover the required.
        refund_deposit(required_storage_in_bytes);
    }

    #[payable]
    pub fn nft_mint(&mut self) {
        let initial_storage_usage = env::storage_usage();
        require!(!self.mint_history.contains_key(&env::predecessor_account_id()), MINTED);
        require!(env::attached_deposit() >= DEFAULT_MINT_FEE, LESS_MINT_FEE);

        let metadata = self.metadata.get().unwrap();
        let level = self.internal_get_new_level(&env::predecessor_account_id(), &metadata, true);
        let metadata = TokenMetadata {
            level
        };
        self.internal_mint(self.operator.clone(), metadata, env::predecessor_account_id(), None);

        // record user
        self.mint_history.insert(&env::predecessor_account_id(), &true);

        let required_storage_in_bytes = env::storage_usage() - initial_storage_usage;
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
        self.internal_wrap_ft_to_nft_with_count(&env::predecessor_account_id(), &env::predecessor_account_id(), &metadata, count.0);
    }
}
