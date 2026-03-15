use crate::error::Error;
use crate::execution::platform_events::core_chain_lock::make_sure_core_is_synced_to_chain_lock::CoreSyncStatus;
use dpp::dashcore::ChainLock;
use dpp::version::PlatformVersion;

use crate::platform_types::platform::Platform;

use crate::rpc::core::CoreRPCLike;

impl<C> Platform<C>
where
    C: CoreRPCLike,
{
    /// Verify the chain lock through core v0
    #[inline(always)]
    pub(super) fn verify_chain_lock_through_core_v0(
        &self,
        chain_lock: &ChainLock,
        submit: bool,
        platform_version: &PlatformVersion,
    ) -> Result<(bool, Option<CoreSyncStatus>), Error> {
        if submit {
            let given_chain_lock_height = chain_lock.block_height;

            let best_chain_locked_height = self.core_rpc.submit_chain_lock(chain_lock)?;
            Ok(if best_chain_locked_height >= given_chain_lock_height {
                (true, Some(CoreSyncStatus::Done))
            } else if best_chain_locked_height - given_chain_lock_height
                <= platform_version
                    .drive_abci
                    .methods
                    .core_chain_lock
                    .recent_block_count_amount
            {
                (true, Some(CoreSyncStatus::Almost))
            } else {
                (true, Some(CoreSyncStatus::Not))
            })
        } else {
            Ok((self.core_rpc.verify_chain_lock(chain_lock)?, None))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rpc::core::{CoreRPCLike, MockCoreRPCLike};
    use dpp::dashcore::hashes::Hash;
    use dpp::dashcore::{BlockHash, ChainLock};

    fn make_chain_lock(height: u32) -> ChainLock {
        ChainLock {
            block_height: height,
            block_hash: BlockHash::all_zeros(),
            signature: [0u8; 96].into(),
        }
    }

    /// Tests that MockCoreRPCLike::verify_chain_lock returns true when configured to do so.
    /// Note: this validates mock wiring, not the production `verify_chain_lock_through_core_v0`
    /// method. The mock cannot easily be injected into a constructed Platform instance.
    #[test]
    fn test_mock_verify_chain_lock_returns_true() {
        let mut mock_rpc = MockCoreRPCLike::new();
        mock_rpc.expect_verify_chain_lock().returning(|_| Ok(true));

        let chain_lock = make_chain_lock(100);
        let result = mock_rpc.verify_chain_lock(&chain_lock);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    /// Tests that MockCoreRPCLike::verify_chain_lock returns false for an invalid lock.
    /// Note: this validates mock wiring, not the production method.
    #[test]
    fn test_mock_verify_chain_lock_returns_false() {
        let mut mock_rpc = MockCoreRPCLike::new();
        mock_rpc.expect_verify_chain_lock().returning(|_| Ok(false));

        let chain_lock = make_chain_lock(100);
        let result = mock_rpc.verify_chain_lock(&chain_lock);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    /// Tests that MockCoreRPCLike::submit_chain_lock returns the same height when configured.
    /// Note: this validates mock wiring, not the production method.
    #[test]
    fn test_mock_submit_chain_lock_same_height_returns_synced() {
        let mut mock_rpc = MockCoreRPCLike::new();
        mock_rpc
            .expect_submit_chain_lock()
            .returning(|cl| Ok(cl.block_height));

        let chain_lock = make_chain_lock(100);
        let best_height = mock_rpc.submit_chain_lock(&chain_lock).unwrap();
        assert_eq!(best_height, 100);
        assert!(best_height >= chain_lock.block_height);
    }

    /// Tests that MockCoreRPCLike::submit_chain_lock returns a higher height.
    /// Note: this validates mock wiring, not the production method.
    #[test]
    fn test_mock_submit_chain_lock_higher_height_returns_synced() {
        let mut mock_rpc = MockCoreRPCLike::new();
        mock_rpc.expect_submit_chain_lock().returning(|_| Ok(150));

        let chain_lock = make_chain_lock(100);
        let best_height = mock_rpc.submit_chain_lock(&chain_lock).unwrap();
        assert!(best_height >= chain_lock.block_height);
    }
}
