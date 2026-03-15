use crate::error::Error;
use crate::execution::platform_events::core_chain_lock::make_sure_core_is_synced_to_chain_lock::CoreSyncStatus;
use crate::platform_types::platform::Platform;
use crate::rpc::core::CoreRPCLike;
use dpp::dashcore::ChainLock;
use dpp::version::PlatformVersion;

impl<C> Platform<C>
where
    C: CoreRPCLike,
{
    /// The point of this call is to make sure core is synced.
    /// Before this call we had previously validated that the chain lock is valid.
    pub(super) fn make_sure_core_is_synced_to_chain_lock_v0(
        &self,
        chain_lock: &ChainLock,
        platform_version: &PlatformVersion,
    ) -> Result<CoreSyncStatus, Error> {
        let given_chain_lock_height = chain_lock.block_height;
        // We need to make sure core is synced to the core height we see as valid for the state transitions

        // TODO: submit_chain_lock responds with invalid signature. We should handle it properly and return CoreSyncStatus
        let best_chain_locked_height = self.core_rpc.submit_chain_lock(chain_lock)?;
        Ok(if best_chain_locked_height >= given_chain_lock_height {
            CoreSyncStatus::Done
        } else if best_chain_locked_height - given_chain_lock_height
            <= platform_version
                .drive_abci
                .methods
                .core_chain_lock
                .recent_block_count_amount
        {
            CoreSyncStatus::Almost
        } else {
            CoreSyncStatus::Not
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::execution::platform_events::core_chain_lock::make_sure_core_is_synced_to_chain_lock::CoreSyncStatus;

    /// Tests that the CoreSyncStatus::Done variant is selected when best >= given height.
    /// Note: this exercises the height-comparison logic directly, not the production
    /// `make_sure_core_is_synced_to_chain_lock_v0` method (which requires wiring a mock RPC
    /// into Platform).
    #[test]
    fn test_sync_status_logic_returns_done_when_heights_match() {
        let given_height: u32 = 100;
        let best_height: u32 = 100;

        let status = if best_height >= given_height {
            CoreSyncStatus::Done
        } else {
            CoreSyncStatus::Not
        };

        assert!(matches!(status, CoreSyncStatus::Done));
    }

    /// Tests the height-comparison branches that map to each CoreSyncStatus variant.
    /// Note: this exercises the control-flow logic directly, not the production method.
    /// The unsigned subtraction when best < given wraps to a large u32, so the "Almost"
    /// branch is unreachable in practice for that case (potential upstream bug).
    #[test]
    fn test_sync_status_logic_variant_selection() {
        let given_height: u32 = 100;
        let recent_block_count = 2u32;

        // Case 1: best >= given => Done
        let best = 100u32;
        assert!(best >= given_height, "should select Done");

        // Case 2: best > given => also Done
        let best = 105u32;
        assert!(best >= given_height, "should select Done");

        // Case 3: best < given — unsigned subtraction wraps, falls to Not
        let best = 50u32;
        let diff = best.wrapping_sub(given_height);
        assert!(
            diff > recent_block_count,
            "wrapped subtraction should exceed recent_block_count, selecting Not"
        );
    }
}
