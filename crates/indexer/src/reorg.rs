use std::collections::VecDeque;

use alloy::primitives::B256;
use alloy::providers::Provider;

/// Sliding-window reorg detector.
///
/// Maintains a window of recent `(block_number, block_hash)` pairs.
/// On each new block, checks whether the parent hash matches the expected chain.
/// If a mismatch is found, returns the block number where the reorg occurred.
pub struct ReorgDetector {
    /// Recent block hashes: (block_number, block_hash)
    window: VecDeque<(u64, B256)>,
    /// Maximum window size
    max_size: usize,
}

impl ReorgDetector {
    pub fn new(max_size: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Check if the new block's parent hash matches our recorded hash for the previous block.
    /// If OK, record the new block hash and return `None`.
    /// If a reorg is detected, return `Some(reorg_block_number)` — the earliest block that diverged.
    ///
    /// `parent_hash` is passed in from the already-fetched block to avoid a redundant RPC call.
    pub async fn check_and_record(
        &mut self,
        block_number: u64,
        block_hash: B256,
        parent_hash: B256,
        provider: &impl Provider,
    ) -> anyhow::Result<Option<u64>> {
        // If we have the parent in our window, verify the hash chain
        if block_number > 0
            && let Some(pos) = self
                .window
                .iter()
                .position(|(num, _)| *num == block_number - 1)
        {
            let (_, expected_parent_hash) = &self.window[pos];

            if parent_hash != *expected_parent_hash {
                tracing::warn!(
                    block_number,
                    expected = %expected_parent_hash,
                    actual = %parent_hash,
                    "Reorg detected: parent hash mismatch"
                );

                // Find the divergence point: walk back through our window
                let reorg_start = self.find_divergence_point(provider).await?;

                // Clear the window from the reorg point onwards
                self.window.retain(|(num, _)| *num < reorg_start);

                return Ok(Some(reorg_start));
            }
        }

        // Record this block
        self.window.push_back((block_number, block_hash));
        if self.window.len() > self.max_size {
            self.window.pop_front();
        }

        Ok(None)
    }

    /// Walk back through the window to find the earliest block where the hash diverges.
    async fn find_divergence_point(&self, provider: &impl Provider) -> anyhow::Result<u64> {
        for (block_number, expected_hash) in self.window.iter().rev() {
            let block = provider.get_block_by_number((*block_number).into()).await?;

            match block {
                Some(b) if b.header.hash == *expected_hash => {
                    // This block is still canonical; reorg starts after it
                    return Ok(*block_number + 1);
                }
                _ => {
                    // This block was also reorged, continue walking back
                    continue;
                }
            }
        }

        // If we walked through the entire window, the reorg is deeper than our window.
        // Return the oldest block in our window as the reorg point.
        Ok(self.window.front().map(|(num, _)| *num).unwrap_or(0))
    }

    /// Current window size (number of tracked blocks).
    pub fn window_size(&self) -> usize {
        self.window.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_detector() {
        let detector = ReorgDetector::new(10);
        assert_eq!(detector.window_size(), 0);
        assert_eq!(detector.max_size, 10);
    }
}
