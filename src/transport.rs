use std::collections::HashMap;

use tonic::codec::CompressionEncoding;

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
        let addr = self.addr_map.get(&peer_id).unwrap().clone();
        let request = tonic::Request::new(msg.into());
        let mut client = KvClient::connect(addr)
            .await
            .unwrap()
            .accept_compressed(CompressionEncoding::Gzip);
        let response = client.append_entries(request).await.unwrap();
        Ok(response.into_inner().into())
    }

    async fn request_votes(
        &self,
        peer_id: crate::raft::PeerId,
        msg: crate::raft::transport::RequestVoteRequest,
    ) -> Result<crate::raft::transport::RequestVoteResponse, Self::Error> {
        let addr = self.addr_map.get(&peer_id).unwrap().clone();
        let request = tonic::Request::new(msg.into());
        let mut client = KvClient::connect(addr)
            .await
            .unwrap()
            .accept_compressed(CompressionEncoding::Gzip);
        let response = client.request_vote(request).await.unwrap();
        Ok(response.into_inner().into())
    }
}
