use std::collections::HashMap;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::{Base64VecU8, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Balance, CryptoHash, PanicOnDefault, Promise, PromiseOrValue, StorageUsage, Gas};

use crate::internal::*;
pub use crate::metadata::*;
pub use crate::mint::*;
pub use crate::nft_core::*;
pub use crate::approval::*;
pub use crate::royalty::*;
pub use crate::events::*;
pub use crate::errors::*;
pub use crate::nep141_ft_core::*;
pub use crate::nep141_ft_internal::*;
pub use crate::nep141_metadata::*;
pub use crate::nep141_storage::*;
pub use crate::nrc404_internal::*;
pub use crate::owner::*;

mod internal;
mod approval;
mod enumeration;
mod metadata;
mod mint;
mod nft_core;
mod royalty;
mod events;
mod errors;
mod nep141_ft_core;
mod nep141_ft_internal;
mod nep141_metadata;
mod nep141_storage;
mod nrc404_internal;
mod owner;

/// This spec can be treated like a version of the standard.
pub const NFT_METADATA_SPEC: &str = "1.0.0";
/// This is the name of the NFT standard we're using
pub const NFT_STANDARD_NAME: &str = "nep171";
pub const MAX_LEVEL_PROBABILITY: u16 = 10000;
pub const DEFAULT_LEVEL: u8 = 1;
pub const MAX_RESERVED_WRAP_GAS: Gas = Gas(Gas::ONE_TERA.0 * 5);
pub const DEFAULT_PROTOCOL_FEE: u128 = 0;
pub const PROTOCOL_FEE_DENOMINATOR: u128 = 1000000;
pub const DEFAULT_MINT_FEE: u128 = 1_5_000_000_000_000_000_000_000; // 0.01Near
pub const NFT_DESCRIPTION: &str = "The lost disk to fully access Deltabot strategies";

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    //contract owner
    pub owner_id: AccountId,
    //contract operator
    pub operator: AccountId,
    pub protocol_fee: u128,
    pub protocol_fee_rate: u128,

    //keeps track of all the token IDs for a given account
    pub tokens_per_owner: LookupMap<AccountId, UnorderedSet<TokenId>>,
    /// level_tokens_per_owner[user][level] = tokenIds
    pub level_tokens_per_owner: LookupMap<AccountId, LookupMap<u8, UnorderedSet<TokenId>>>,

    //keeps track of the token struct for a given token ID
    pub tokens_by_id: LookupMap<TokenId, Token>,

    //keeps track of the token metadata for a given token ID
    pub token_metadata_by_id: UnorderedMap<TokenId, TokenMetadata>,

    //keeps track of the metadata for the contract
    pub metadata: LazyOption<NFTContractMetadata>,
    pub mediadata: LazyOption<NFTMediaData>,

    // pub ft: FungibleToken,

    pub next_nft_id: u128,

    /// Keep track of each account's balances
    pub accounts: LookupMap<AccountId, Balance>,

    /// Total supply of all tokens.
    pub total_supply: Balance,

    /// The bytes for the largest possible account ID that can be registered on the contract
    pub bytes_for_longest_account_id: StorageUsage,

    /// mint_white_list[address] = true/false
    pub mint_white_list: LookupMap<AccountId, bool>,
    /// fee_white_list[address] = true/false
    pub fee_white_list: LookupMap<AccountId, bool>,

    pub mint_history: LookupMap<AccountId, bool>,
}

/// Helper structure for keys of the persistent collections.
#[derive(BorshSerialize)]
pub enum StorageKey {
    TokensPerOwner,
    TokenPerOwnerInner { account_id_hash: CryptoHash },
    LevelTokensPerOwner,
    LevelTokensPerOwnerLevel { account_id_hash: CryptoHash },
    LevelTokensPerOwnerLevelInner { account_id_level_hash: CryptoHash },
    TokensById,
    TokenMetadataById,
    NFTContractMetadata,
    TokensPerType,
    TokensPerTypeInner { token_type_hash: CryptoHash },
    TokenTypesLocked,
    FTToken,
    Accounts,
    Metadata,
    NFTMediaData,
}

#[near_bindgen]
impl Contract {
    /*
        initialization function (can only be called once).
        this initializes the contract with metadata that was passed in and
        the owner_id.
    */
    #[init]
    pub fn new(owner_id: AccountId, metadata: NFTContractMetadata, mediadata: NFTMediaData, total_supply: U128) -> Self {
        // check metadata
        Contract::internal_check_contract_meta_data(&metadata, &mediadata);

        //create a variable of type Self with all the fields initialized.
        let mut contract = Self {
            //Storage keys are simply the prefixes used for the collections. This helps avoid data collision
            tokens_per_owner: LookupMap::new(StorageKey::TokensPerOwner.try_to_vec().unwrap()),
            level_tokens_per_owner: LookupMap::new(StorageKey::LevelTokensPerOwner.try_to_vec().unwrap()),
            tokens_by_id: LookupMap::new(StorageKey::TokensById.try_to_vec().unwrap()),
            token_metadata_by_id: UnorderedMap::new(
                StorageKey::TokenMetadataById.try_to_vec().unwrap(),
            ),
            //set the owner_id field equal to the passed in owner_id.
            owner_id: owner_id.clone(),
            operator: owner_id.clone(),
            protocol_fee: 0,
            protocol_fee_rate: DEFAULT_PROTOCOL_FEE,
            metadata: LazyOption::new(
                StorageKey::NFTContractMetadata.try_to_vec().unwrap(),
                Some(&metadata),
            ),
            mediadata: LazyOption::new(
                StorageKey::NFTMediaData.try_to_vec().unwrap(),
                Some(&mediadata),
            ),
            next_nft_id: 0,
            // Set the total supply
            total_supply: total_supply.0,
            // Set the bytes for the longest account ID to 0 temporarily until it's calculated later
            bytes_for_longest_account_id: 0,
            // Storage keys are simply the prefixes used for the collections. This helps avoid data collision
            accounts: LookupMap::new(StorageKey::Accounts.try_to_vec().unwrap()),
            mint_white_list: LookupMap::new(b"mint_white".to_vec()),
            fee_white_list: LookupMap::new(b"fee_white".to_vec()),
            mint_history: LookupMap::new(b"mint_his".to_vec()),
        };

        // Measure the bytes for the longest account ID and store it in the contract.
        contract.measure_bytes_for_longest_account_id();

        // Register the owner's account and set their balance to the total supply.
        contract.internal_register_account(&owner_id);
        contract.internal_deposit(&owner_id, total_supply.into());

        // Emit an event showing that the FTs were minted
        FtMint {
            owner_id: &owner_id,
            amount: &total_supply,
            memo: Some("Initial token supply is minted"),
        }.emit();

        //return the Contract object
        contract
    }

    // pub fn update_media(&mut self, metadata: NFTContractMetadata) {
    //     require!(env::predecessor_account_id() == self.owner_id);
    //     self.metadata = LazyOption::new(
    //         StorageKey::NFTContractMetadata.try_to_vec().unwrap(),
    //         Some(&metadata),
    //     );
    // }
}

#[cfg(test)]
mod tests;
