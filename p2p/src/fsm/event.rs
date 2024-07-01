//! State machine events.
use std::sync::Arc;
use std::{error, fmt, io, net};

use nakamoto_common::bitcoin::network::address::Address;
use nakamoto_common::bitcoin::network::constants::ServiceFlags;
use nakamoto_common::bitcoin::network::message::NetworkMessage;
use nakamoto_common::bitcoin::network::message_bloom::FilterLoad;
use nakamoto_common::bitcoin::{MerkleBlock, Transaction, Txid};
use nakamoto_common::block::filter::BlockFilter;
use nakamoto_common::block::{Block, BlockHash, BlockHeader, Height};
use nakamoto_common::nonempty::NonEmpty;
use nakamoto_common::p2p::peer::Source;
use nakamoto_net::Disconnect;

use crate::fsm;
use crate::fsm::fees::FeeEstimate;
use crate::fsm::{Link, LocalTime, PeerId};

/// Event emitted by the client, after the "loading" phase is over.
#[derive(Debug, Clone)]
pub enum Event {
    /// The node is initializing its state machine and about to start network activity.
    Initializing,
    /// Ready to process peer events and start receiving commands.
    /// Note that this isn't necessarily the first event emitted.
    Ready {
        /// The tip of the block header chain.
        tip: Height,
        /// The tip of the filter header chain.
        filter_tip: Height,
        /// Local time.
        time: LocalTime,
    },
    /// A BloomFilter was loaded to a peer
    PeerLoadedBloomFilter {
        /// the filter loaded to peer
        filter: FilterLoad,
        /// Peer address.
        peer: PeerId,
    },

    /// Peer connected. This is fired when the physical TCP/IP connection
    /// is established. Use [`Event::PeerNegotiated`] to know when the P2P handshake
    /// has completed.
    PeerConnected {
        /// Peer address.
        addr: PeerId,
        /// Local address.
        local_addr: net::SocketAddr,
        /// Connection link.
        link: Link,
    },
    /// Outbound peer connection initiated.
    PeerConnecting {
        /// Peer address.
        addr: PeerId,
        /// Address source.
        source: Source,
        /// Peer services.
        services: ServiceFlags,
    },
    /// Peer disconnected after successful connection.
    PeerDisconnected {
        /// Peer address.
        addr: PeerId,
        /// Reason for disconnection.
        reason: Disconnect<fsm::DisconnectReason>,
    },
    /// Peer timed out when waiting for response.
    /// This usually leads to a disconnection.
    PeerTimedOut {
        /// Peer address.
        addr: PeerId,
    },
    /// Connection was never established and timed out or failed.
    PeerConnectionFailed {
        /// Peer address.
        addr: PeerId,
        /// Connection error.
        error: Arc<io::Error>,
    },
    /// Peer handshake completed. The peer connection is fully functional from this point.
    PeerNegotiated {
        /// Peer address.
        addr: PeerId,
        /// Connection link.
        link: Link,
        /// Peer services.
        services: ServiceFlags,
        /// Whether this is a persistent peer.
        persistent: bool,
        /// Peer height.
        height: Height,
        /// Address of our node, as seen by remote.
        receiver: Address,
        /// Peer user agent.
        user_agent: String,
        /// Negotiated protocol version.
        version: u32,
        /// Transaction relay.
        relay: bool,
    },
    /// The best known height amongst connected peers has been updated.
    /// Note that there is no guarantee that this height really exists;
    /// peers don't have to follow the protocol and could send a bogus
    /// height.
    PeerHeightUpdated {
        /// Best block height known.
        height: Height,
    },
    /// A peer misbehaved.
    PeerMisbehaved {
        /// Peer address.
        addr: PeerId,
        /// Reason of misbehavior.
        reason: &'static str,
    },
    /// A block was added to the main chain.
    BlockConnected {
        /// Block header.
        header: BlockHeader,
        /// Height of the block.
        height: Height,
    },
    /// One of the blocks of the main chain was reverted, due to a re-org.
    /// These events will fire from the latest block starting from the tip, to the earliest.
    /// Mark all transactions belonging to this block as *unconfirmed*.
    BlockDisconnected {
        /// Header of the block.
        header: BlockHeader,
        /// Height of the block when it was part of the main chain.
        height: Height,
    },
    /// Block downloaded and processed by inventory manager.
    BlockProcessed {
        /// The full block.
        block: Block,
        /// The block height.
        height: Height,
        /// The fee estimate for this block.
        fees: Option<FeeEstimate>,
    },
    /// A block has matched one of the filters and is ready to be processed.
    /// This event usually precedes [`Event::TxStatusChanged`] events.
    BlockMatched {
        /// Block height.
        height: Height,
        /// Matching block.
        block: Block,
    },

    /// We received a merkle block from the network.
    ReceivedMerkleBlock {
        /// Block height.
        height: Height,
        /// Matching block.
        merkle_block: MerkleBlock,
        /// the peer who sent us the block
        peer: PeerId,
    },
    /// Block header chain is in sync with network.
    BlockHeadersSynced {
        /// Block height.
        height: Height,
        /// Chain tip.
        hash: BlockHash,
    },
    /// Block headers imported. Emitted when headers are fetched from peers,
    /// or imported by the user.
    BlockHeadersImported {
        /// New tip hash.
        hash: BlockHash,
        /// New tip height.
        height: Height,
        /// Block headers connected to the active chain.
        connected: NonEmpty<(Height, BlockHeader)>,
        /// Block headers reverted from the active chain.
        reverted: Vec<(Height, BlockHeader)>,
        /// Set if this import triggered a chain reorganization.
        reorg: bool,
    },
    /// BlockFilter Imported
    BlockFilterImported {
        /// New tip hash.
        hash: BlockHash,
        /// New tip height.
        height: Height,
        /// Block headers connected to the active chain.
        connected: NonEmpty<(Height, BlockHeader)>,
        /// Block headers reverted from the active chain.
        reverted: Vec<(Height, BlockHeader)>,
        /// Set if this import triggered a chain reorganization.
        reorg: bool,
    },
    /// Transaction fee rate estimated for a block.
    FeeEstimated {
        /// Block hash of the estimate.
        block: BlockHash,
        /// Block height of the estimate.
        height: Height,
        /// Fee estimate.
        fees: FeeEstimate,
    },
    /// A filter was processed. If it matched any of the scripts in the watchlist,
    /// the corresponding block was scheduled for download, and a [`Event::BlockMatched`]
    /// event will eventually be fired.
    FilterProcessed {
        /// Corresponding block hash.
        block: BlockHash,
        /// Filter height (same as block).
        height: Height,
        /// Whether or not this filter matched any of the watched scripts.
        matched: bool,
        /// Whether or not this filter is valid.
        // TODO: Do not emit event for invalid filter.
        valid: bool,
        /// Filter was cached.
        cached: bool,
    },
    /// A filter was received.
    FilterReceived {
        /// Peer we received from.
        from: PeerId,
        /// The received filter.
        filter: BlockFilter,
        /// Filter height.
        height: Height,
        /// Hash of corresponding block.
        block: BlockHash,
    },
    /// A filter rescan has started.
    FilterRescanStarted {
        /// Start height.
        start: Height,
        /// End height.
        stop: Option<Height>,
    },
    /// A filter rescan has stopped.
    FilterRescanStopped {
        /// Stop height.
        height: Height,
    },
    /// A merkle block rescan has stopped.
    MerkleBlockRescanStopped {
        /// Stop height.
        height: Height,
        /// peer
        peer: PeerId,
    },
    /// A merkle block rescan has started.
    MerkleBlockScanStarted {
        /// Start height.
        start: Height,
        /// End height.
        stop: Option<Height>,
        /// peer
        peer: PeerId,
    },
    /// Filter headers synced up to block header height.
    FilterHeadersSynced {
        /// Block height.
        height: Height,
    },
    /// The status of a transaction has changed.
    TxStatusChanged {
        /// The Transaction ID.
        txid: Txid,
        /// The new transaction status.
        status: TxStatus,
    },
    /// A matched transaction was receiced.
    ReceivedMatchedTx {
        /// The Transaction.
        transaction: Transaction,
    },
    /// Scanned the chain up to a certain height.
    Scanned {
        /// Height up to which we've scanned and processed blocks.
        height: Height,
    },
    /// A gossip message was received from a peer.
    MessageReceived {
        /// Peer that sent the message.
        from: PeerId,
        /// Message payload.
        message: Arc<NetworkMessage>,
    },
    /// Address book exhausted.
    AddressBookExhausted,
    /// An error occured.
    Error {
        /// Error source.
        error: Arc<dyn error::Error + 'static + Sync + Send>,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Initializing => {
                write!(fmt, "Initializing peer-to-peer system..")
            }

            Self::PeerLoadedBloomFilter { filter, peer } => {
                _ = filter;
                write!(fmt, "Bloom filter loaded to peer {}", peer)
            }
            Self::MerkleBlockScanStarted { start, .. } => {
                write!(fmt, "A merkle block rescan started at height {start}")
            }
            Self::MerkleBlockRescanStopped { height, .. } => {
                write!(fmt, "A merkle block resan stopped {height}")
            }

            Self::Ready { .. } => {
                write!(fmt, "Ready to process events and commands")
            }
            Self::BlockHeadersSynced { height, hash } => {
                write!(
                    fmt,
                    "Chain in sync with network at height {height} ({hash})"
                )
            }
            Self::ReceivedMerkleBlock {
                height,
                merkle_block,
                peer,
            } => {
                _ = merkle_block;
                // let hash = merkle_block.header.block_hash();
                write!(fmt, "MerkleBlock received at height {height} from {peer}")
            }
            Self::BlockFilterImported {
                hash,
                height,
                reorg,
                ..
            } => {
                write!(
                    fmt,
                    "Block Filters imported to {hash} at height {height} (reorg={reorg})"
                )
            }
            Self::BlockHeadersImported {
                hash,
                height,
                reorg,
                ..
            } => {
                write!(
                    fmt,
                    "Chain tip updated to {hash} at height {height} (reorg={reorg})"
                )
            }
            Self::BlockConnected { header, height, .. } => {
                write!(
                    fmt,
                    "Block {} connected at height {}",
                    header.block_hash(),
                    height
                )
            }
            Self::BlockDisconnected { header, height, .. } => {
                write!(
                    fmt,
                    "Block {} disconnected at height {}",
                    header.block_hash(),
                    height
                )
            }
            Self::BlockProcessed { block, height, .. } => {
                write!(
                    fmt,
                    "Block {:?} processed at height {}",
                    block.block_hash(),
                    height
                )
            }

            Self::BlockMatched { height, .. } => {
                write!(fmt, "Block matched at height {}", height)
            }
            Self::FeeEstimated { fees, height, .. } => {
                write!(
                    fmt,
                    "Transaction median fee rate for block #{} is {} sat/vB",
                    height, fees.median,
                )
            }
            Self::FilterRescanStarted {
                start,
                stop: Some(stop),
            } => {
                write!(fmt, "Rescan started from height {start} to {stop}")
            }
            Self::FilterRescanStarted { start, stop: None } => {
                write!(fmt, "Rescan started from height {start}")
            }
            Self::FilterRescanStopped { height } => {
                write!(fmt, "Rescan completed at height {height}")
            }
            Self::FilterHeadersSynced { height } => {
                write!(fmt, "Filter headers synced up to height {height}")
            }
            Self::FilterReceived { from, block, .. } => {
                write!(fmt, "Filter for block {block} received from {from}")
            }
            Self::FilterProcessed {
                height, matched, ..
            } => {
                write!(
                    fmt,
                    "Filter processed at height {} (match = {})",
                    height, matched
                )
            }
            Self::TxStatusChanged { txid, status } => {
                write!(fmt, "Transaction {} status changed: {}", txid, status)
            }
            Self::Scanned { height, .. } => write!(fmt, "Chain scanned up to height {height}"),
            Self::PeerConnected { addr, link, .. } => {
                write!(fmt, "Peer {} connected ({:?})", &addr, link)
            }
            Self::PeerConnectionFailed { addr, error } => {
                write!(
                    fmt,
                    "Peer connection attempt to {} failed with {}",
                    &addr, error
                )
            }
            Self::PeerHeightUpdated { height } => {
                write!(fmt, "Peer height updated to {}", height)
            }
            Self::ReceivedMatchedTx { transaction } => {
                write!(fmt, "Received transaction match {}", transaction.txid())
            }
            Self::PeerMisbehaved { addr, reason } => {
                write!(fmt, "Peer {addr} misbehaved: {reason}")
            }
            Self::PeerDisconnected { addr, reason } => {
                write!(fmt, "Disconnected from {} ({})", &addr, reason)
            }
            Self::PeerTimedOut { addr } => {
                write!(fmt, "Peer {addr} timed out")
            }
            Self::PeerConnecting { addr, .. } => {
                write!(fmt, "Connecting to peer {addr}")
            }
            Self::PeerNegotiated {
                addr,
                height,
                services,
                ..
            } => write!(
                fmt,
                "Peer {} negotiated with services {} and height {}..",
                addr, services, height
            ),
            Self::MessageReceived { from, message } => {
                write!(fmt, "Message `{}` received from {from}", message.cmd())
            }
            Self::AddressBookExhausted => {
                write!(
                    fmt,
                    "Address book exhausted.. fetching new addresses from peers"
                )
            }
            Self::Error { error } => {
                write!(fmt, "Error: {error}")
            }
        }
    }
}

/// Transaction status of a given transaction.
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum TxStatus {
    /// This is the initial state of a transaction after it has been announced by the
    /// client.
    Unconfirmed,
    /// Transaction was acknowledged by a peer.
    ///
    /// This is the case when a peer requests the transaction data from us after an inventory
    /// announcement. It does not mean the transaction is considered valid by the peer.
    Acknowledged {
        /// Peer acknowledging the transaction.
        peer: net::SocketAddr,
    },
    /// Transaction was included in a block. This event is fired after
    /// a block from the main chain is scanned.
    Confirmed {
        /// Height at which it was included.
        height: Height,
        /// Hash of the block in which it was included.
        block: BlockHash,
    },
    /// A transaction that was previously confirmed, and is now reverted due to a
    /// re-org. Note that this event can only fire if the originally confirmed tx
    /// is still in memory.
    Reverted {
        /// The reverted transaction.
        transaction: Transaction,
    },
    /// Transaction was replaced by another transaction, and will probably never
    /// be included in a block. This can happen if an RBF transaction is replaced by one with
    /// a higher fee, or if a transaction is reverted and a conflicting transaction replaces
    /// it. In this case it would be preceded by a [`TxStatus::Reverted`] status.
    Stale {
        /// Transaction replacing the given transaction and causing it to be stale.
        replaced_by: Txid,
        /// Block of the included transaction.
        block: BlockHash,
    },
}

impl fmt::Display for TxStatus {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unconfirmed => write!(fmt, "transaction is unconfirmed"),
            Self::Acknowledged { peer } => {
                write!(fmt, "transaction was acknowledged by peer {}", peer)
            }
            Self::Confirmed { height, block } => write!(
                fmt,
                "transaction was included in block {} at height {}",
                block, height
            ),
            Self::Reverted { transaction } => {
                write!(fmt, "transaction {} has been reverted", transaction.txid())
            }
            Self::Stale { replaced_by, block } => write!(
                fmt,
                "transaction was replaced by {} in block {}",
                replaced_by, block
            ),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use nakamoto_common::bitcoin_hashes::Hash;
    use nakamoto_test::block::gen;

    #[test]
    fn test_tx_status_ordering() {
        assert!(
            TxStatus::Unconfirmed
                < TxStatus::Acknowledged {
                    peer: ([0, 0, 0, 0], 0).into()
                }
        );
        assert!(
            TxStatus::Acknowledged {
                peer: ([0, 0, 0, 0], 0).into()
            } < TxStatus::Confirmed {
                height: 0,
                block: BlockHash::all_zeros(),
            }
        );
        assert!(
            TxStatus::Confirmed {
                height: 0,
                block: BlockHash::all_zeros(),
            } < TxStatus::Reverted {
                transaction: gen::transaction(&mut fastrand::Rng::new())
            }
        );
        assert!(
            TxStatus::Reverted {
                transaction: gen::transaction(&mut fastrand::Rng::new())
            } < TxStatus::Stale {
                replaced_by: Txid::all_zeros(),
                block: BlockHash::all_zeros()
            }
        );
    }
}
