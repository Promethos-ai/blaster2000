//! Distributed Inference Client Library
//! 
//! Provides DHT-based node discovery and distributed inference client functionality.
//! 
//! Nodes announce themselves to the DHT, and clients discover nodes dynamically.

pub mod distributed_client;
pub mod dht_discovery;
pub mod node_announcement;

pub use distributed_client::{
    DistributedInferenceClient,
    InferenceParams,
    InferenceRequest,
    InferenceResponse,
    Message,
    ResponseMetadata,
    ClusterInfo,
    NodeConfig,
    ClusterConfig,
};

pub use dht_discovery::{
    DHTClient,
    TrackerDHTClient,
    NodeDiscovery,
    DiscoveredNode,
    NodeInfoResponse,
    ModelInfo,
};

pub use node_announcement::NodeAnnouncer;
