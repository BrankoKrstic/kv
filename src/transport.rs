use std::collections::HashMap;

use crate::{
    kv::KVCommand,
    proto::{kv_client::KvClient, AppendEntriesGrpcRequest, AppendEntriesGrpcResponse},
    raft::{transport::Transport, PeerId},
};

pub struct GrpcTransport {
    addr_map: HashMap<PeerId, String>,
}

impl GrpcTransport {
    pub fn new(addr_map: HashMap<PeerId, String>) -> Self {
        Self { addr_map }
    }
}

impl Transport<KVCommand> for GrpcTransport {
    type Error = std::io::Error;

    async fn append_entries(
        &self,
        peer_id: crate::raft::PeerId,
        msg: crate::raft::transport::AppendEntriesRequest<KVCommand>,
    ) -> Result<crate::raft::transport::AppendEntriesResponse, Self::Error> {
        let addr = self.addr_map.get(&peer_id).unwrap();
        let request = tonic::Request::new(AppendEntriesGrpcRequest {
            term: todo!(),
            leader_id: todo!(),
            prev_log_index: todo!(),
            prev_log_term: todo!(),
            entries: todo!(),
            leader_commit: todo!(),
        });
        todo!()
    }

    async fn request_votes(
        &self,
        peer_id: crate::raft::PeerId,
        msg: crate::raft::transport::RequestVoteRequest,
    ) -> Result<crate::raft::transport::RequestVoteResponse, Self::Error> {
        todo!()
    }
}
