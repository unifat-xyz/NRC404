use crate::*;
use near_sdk::{require};
use sha2::{Sha256, Digest};

impl Contract {

    pub(crate) fn internal_check_contract_meta_data(metadata: &NFTContractMetadata) {
        if !metadata.enable_random_level {
            return;
        }
        require!(metadata.level_probability.is_some(), INVALID_LEVEL_INITIAL);
        require!(metadata.max_level == (metadata.level_probability.clone().unwrap().len() as u8), INVALID_LEVEL_INITIAL);
        let mut total_probability = 0 as u16;
        for probability in metadata.level_probability.clone().unwrap() {
            total_probability += probability;
        }
        require!(total_probability == MAX_LEVEL_PROBABILITY, INVALID_LEVEL_INITIAL);
    }

    pub(crate) fn internal_get_and_use_next_nft_id(&mut self) -> u128 {
        let next_nft_id = self.next_nft_id;
        self.next_nft_id += 1;
        return next_nft_id;
    }

    pub(crate) fn internal_wrap_nft_to_ft(&mut self, account_id: &AccountId, less_amount: Balance) {
        require!(self.tokens_per_owner.contains_key(account_id), LESS_BALANCE);
        let token_set = self.tokens_per_owner.get(account_id).unwrap();
        let metadata = self.metadata.get().unwrap();
        let decimal_int = 10u128.pow(metadata.decimals as u32);
        require!(token_set.len() as u128 * decimal_int >= less_amount, LESS_BALANCE);
        // wrap from low level nft
        let level_token_set = self.level_tokens_per_owner.get(account_id).unwrap();
        let mut need_amount = less_amount;
        let mut need_del_token_id = vec![];
        let mut need_del_token_level = vec![];
        for i in 1..(metadata.max_level+1) {
            let level_token_ids_op = level_token_set.get(&i);
            if level_token_ids_op.is_none() {
                continue;
            }
            let level_token_ids = level_token_ids_op.unwrap();
            for token_id in level_token_ids.iter() {
                need_del_token_id.push(token_id.clone());
                need_del_token_level.push(i);
                if need_amount >= decimal_int {
                    need_amount -= decimal_int;
                } else {
                    need_amount = 0;
                    break;
                }
            }
            if need_amount == 0 {
                break;
            }
        }
        if need_del_token_id.len() == 0 {
            return;
        }
        for (index, del_token_id) in need_del_token_id.iter().enumerate() {
            self.internal_remove_token_from_owner(account_id, &del_token_id, &need_del_token_level.get(index).unwrap());
            self.tokens_by_id.remove(&del_token_id);
            self.token_metadata_by_id.remove(&del_token_id);
            // add balance
            self.internal_deposit(account_id, decimal_int);

            // Construct the mint log as per the events standard.
            let nft_burn_log: EventLog = EventLog {
                // Standard name ("nep171").
                standard: NFT_STANDARD_NAME.to_string(),
                // Version of the standard ("nft-1.0.0").
                version: NFT_METADATA_SPEC.to_string(),
                // The data related with the event stored in a vector.
                event: EventLogVariant::NftBurn(vec![NftMintLog {
                    // Owner of the token.
                    owner_id: account_id.to_string(),
                    // Vector of token IDs that were minted.
                    token_ids: vec![del_token_id.to_string()],
                    // An optional memo to include.
                    memo: None,
                }]),
            };

            // Log the serialized json.
            env::log_str(&nft_burn_log.to_string());
        }
    }

    pub(crate) fn internal_wrap_ft_to_nft_with_count(&mut self, account_id: &AccountId, metadata: &NFTContractMetadata, count: u128) {
        for _ in 0..count {
            if self.internal_get_remaining_gas() < MAX_RESERVED_WRAP_GAS.0 {
                break;
            }
            let metadata = TokenMetadata {
                level: self.internal_get_new_level(account_id, &metadata), title: None, description: None,
                media: None, media_hash: None, copies: None, issued_at: Some(env::block_timestamp_ms()), expires_at: None,
                starts_at: None, updated_at: Some(env::block_timestamp_ms()), extra: None, reference: None, reference_hash: None,
            };
            self.internal_mint(account_id.clone(), metadata, account_id.clone(), None);
        }
    }

    pub(crate) fn internal_wrap_ft_to_nft(&mut self, account_id: &AccountId) {
        let ft_balance = self.internal_unwrap_balance_of(&account_id);
        let metadata = self.metadata.get().unwrap();
        let decimal_int = 10u128.pow(metadata.decimals as u32);
        if ft_balance < decimal_int {
            // not need to wrap to nft
            return;
        }
        let wrap_count = ft_balance / decimal_int;
        self.internal_wrap_ft_to_nft_with_count(account_id, &metadata, wrap_count);
    }

    pub(crate) fn internal_get_new_level(&self, account_id: &AccountId, metadata: &NFTContractMetadata) -> u8 {
        if !metadata.enable_random_level {
            return DEFAULT_LEVEL;
        }
        let random = self.pseudo_random_number(&account_id.to_string(), MAX_LEVEL_PROBABILITY as u64);
        let mut added_probability = 0 as u16;
        for (index, probability) in metadata.level_probability.clone().unwrap().iter().enumerate() {
            added_probability += probability;
            if (random as u16) < added_probability {
                return (index + 1) as u8;
            }
        }
        return DEFAULT_LEVEL;
    }

    pub(crate) fn internal_mint(&mut self, operator: AccountId, metadata: TokenMetadata, receiver_id: AccountId, perpetual_royalties: Option<HashMap<AccountId, u32>>) {
        let nft_metadata = self.metadata.get().unwrap();
        require!(metadata.level != 0 && metadata.level <= nft_metadata.max_level, INVALID_LEVEL);

        // create a royalty map to store in the token
        let mut royalty = HashMap::new();

        // if perpetual royalties were passed into the function:
        if let Some(perpetual_royalties) = perpetual_royalties {
            //make sure that the length of the perpetual royalties is below 7 since we won't have enough GAS to pay out that many people
            assert!(perpetual_royalties.len() < 7, "Cannot add more than 6 perpetual royalty amounts");

            //iterate through the perpetual royalties and insert the account and amount in the royalty map
            for (account, amount) in perpetual_royalties {
                royalty.insert(account, amount);
            }
        }

        //specify the token struct that contains the owner ID
        let token = Token {
            //set the owner ID equal to the receiver ID passed into the function
            owner_id: receiver_id,
            //we set the approved account IDs to the default value (an empty map)
            approved_account_ids: Default::default(),
            //the next approval ID is set to 0
            next_approval_id: 0,
            //the map of perpetual royalties for the token (The owner will get 100% - total perpetual royalties)
            royalty,
        };

        // cost 1 token
        self.internal_withdraw(&operator, 10u128.pow(nft_metadata.decimals as u32));

        let token_id = self.internal_get_and_use_next_nft_id().to_string();
        self.tokens_by_id.insert(&token_id, &token);

        //insert the token ID and metadata
        self.token_metadata_by_id.insert(&token_id, &metadata);

        //call the internal method for adding the token to the owner
        self.internal_add_token_to_owner(&token.owner_id, &token_id, &metadata.level);

        // Construct the mint log as per the events standard.
        let nft_mint_log: EventLog = EventLog {
            // Standard name ("nep171").
            standard: NFT_STANDARD_NAME.to_string(),
            // Version of the standard ("nft-1.0.0").
            version: NFT_METADATA_SPEC.to_string(),
            // The data related with the event stored in a vector.
            event: EventLogVariant::NftMint(vec![NftMintLog {
                // Owner of the token.
                owner_id: token.owner_id.to_string(),
                // Vector of token IDs that were minted.
                token_ids: vec![token_id.to_string()],
                // An optional memo to include.
                memo: None,
            }]),
        };

        // Log the serialized json.
        env::log_str(&nft_mint_log.to_string());
    }

    pub fn internal_get_remaining_gas(&self) -> u64 {
        let prepaid_gas = env::prepaid_gas();
        let used_gas = env::used_gas();
        (prepaid_gas - used_gas).0
    }

    pub fn pseudo_random_number(&self, account_id: &str, n: u64) -> u64 {
        let block_index = env::block_height();
        let block_timestamp = env::block_timestamp();
        let account_balance = env::account_balance();
        let epoch_height = env::epoch_height();
        let random_seed = env::random_seed();
        let used_gas = env::used_gas();

        let mut hasher = Sha256::new();

        hasher.update(block_index.to_le_bytes());
        hasher.update(block_timestamp.to_le_bytes());
        hasher.update(account_balance.to_le_bytes());
        hasher.update(epoch_height.to_le_bytes());
        hasher.update(&random_seed);
        hasher.update(used_gas.0.to_le_bytes());
        hasher.update(account_id.as_bytes());

        let result = hasher.finalize();
        let hash_number = u64::from_le_bytes([
            result[0].clone(), result[1].clone(), result[2].clone(), result[3].clone(),
            result[4].clone(), result[5].clone(), result[6].clone(), result[7].clone(),
        ]);

        let combined = block_index
            .wrapping_add(block_timestamp)
            .wrapping_add(hash_number)
            .wrapping_add(epoch_height)
            .wrapping_add(used_gas.0);
        let balance_as_u64 = (account_balance % u128::from(u64::MAX)) as u64;
        let combined_with_balance = combined.wrapping_add(balance_as_u64);
        let complex_hash = combined_with_balance.wrapping_mul(0x4cd6944c5e2e53a9);

        let random_number = complex_hash % n;
        random_number
    }

}
