//! Processing asset lock transactions for identity registration detection
//!
//! This module handles the detection and fetching of identities created from
//! asset lock transactions.

use super::key_derivation::derive_identity_auth_key_hash;
use super::PlatformWalletInfo;
use crate::error::PlatformWalletError;
use dpp::identity::accessors::IdentityGettersV0;
use dpp::prelude::Identifier;
use key_wallet::wallet::managed_wallet_info::wallet_info_interface::WalletInfoInterface;

impl PlatformWalletInfo {
    /// Discover identity and fetch contact requests for a single asset lock transaction
    ///
    /// This is called automatically when an asset lock transaction is detected.
    ///
    /// # Arguments
    ///
    /// * `wallet` - The wallet to derive authentication keys from
    /// * `tx` - The asset lock transaction
    ///
    /// # Returns
    ///
    /// Returns Ok(Some(identity_id)) if found, Ok(None) if not found
    pub async fn fetch_identity_and_contacts_for_asset_lock(
        &mut self,
        wallet: &key_wallet::Wallet,
        tx: &dashcore::Transaction,
    ) -> Result<Option<Identifier>, PlatformWalletError> {
        let result = self
            .fetch_contact_requests_for_identities_after_asset_locks(
                wallet,
                std::slice::from_ref(tx),
            )
            .await?;

        Ok(result.first().copied())
    }

    /// Discover identities and fetch contact requests after asset locks
    ///
    /// When asset lock transactions are seen (added as immature), identities may have been registered.
    /// This searches for the first identity key to discover newly registered identities
    /// and fetches their DashPay contact requests.
    ///
    /// # Arguments
    ///
    /// * `wallet` - The wallet to derive authentication keys from
    /// * `asset_lock_transactions` - List of asset lock transactions
    ///
    /// # Returns
    ///
    /// Returns a list of identity IDs for which contact requests were fetched
    pub async fn fetch_contact_requests_for_identities_after_asset_locks(
        &mut self,
        wallet: &key_wallet::Wallet,
        asset_lock_transactions: &[dashcore::Transaction],
    ) -> Result<Vec<Identifier>, PlatformWalletError> {
        use dash_sdk::platform::types::identity::PublicKeyHash;
        use dash_sdk::platform::Fetch;

        let mut identities_processed = Vec::new();

        // Early return if no asset lock transactions
        if asset_lock_transactions.is_empty() {
            return Ok(identities_processed);
        }

        // Get SDK from identity manager
        let sdk = self
            .identity_manager()
            .sdk
            .as_ref()
            .ok_or_else(|| {
                PlatformWalletError::InvalidIdentityData(
                    "SDK not configured in identity manager".to_string(),
                )
            })?
            .clone();

        // Derive the first authentication key hash (identity_index 0, key_index 0)
        let key_hash_array = derive_identity_auth_key_hash(wallet, self.network(), 0, 0)?;

        // Query Platform for identity by public key hash
        match dpp::identity::Identity::fetch(&sdk, PublicKeyHash(key_hash_array)).await {
            Ok(Some(identity)) => {
                let identity_id = identity.id();

                // Add identity to manager if not already present
                if !self
                    .identity_manager()
                    .identities()
                    .contains_key(&identity_id)
                {
                    self.identity_manager_mut().add_identity(identity.clone())?;
                }

                // Fetch DashPay contact requests for this identity
                if let Err(e) = self
                    .fetch_and_store_contact_requests(&sdk, &identity, &identity_id)
                    .await
                {
                    tracing::warn!(
                        %identity_id,
                        "Failed to fetch contact requests after asset lock: {}",
                        e
                    );
                } else {
                    identities_processed.push(identity_id);
                }
            }
            Ok(None) => {
                // No identity found for this key - that's ok, may not be registered yet
            }
            Err(e) => {
                tracing::warn!("Failed to query identity by public key hash: {}", e);
            }
        }

        Ok(identities_processed)
    }
}
