use std::{collections::HashMap, error::Error, fmt::Display};

use tonic::{codec::CompressionEncoding, transport::Channel};

use crate::{
    kv::KVCommand,
    proto::kv_client::KvClient,
    raft::{transport::Transport, PeerId},
};

pub struct GrpcTransport {
    addr_map: HashMap<PeerId, KvClient<Channel>>,
}

impl GrpcTransport {
    pub fn new(addr_map: HashMap<PeerId, String>) -> Self {
        let channel_map = addr_map
            .into_iter()
            .map(|x| {
                let channel = Channel::builder(x.1.parse().unwrap()).connect_lazy();
                let client = KvClient::new(channel)
                    .accept_compressed(CompressionEncoding::Gzip)
                    .send_compressed(CompressionEncoding::Gzip);
                (x.0, client)
            })
            .collect();
        Self {
            addr_map: channel_map,
        }
    }
}

#[derive(Debug)]
pub enum GrpcError {
    TransportError(tonic::transport::Error),
    Status(tonic::Status),
}

impl From<tonic::transport::Error> for GrpcError {
    fn from(value: tonic::transport::Error) -> Self {
        GrpcError::TransportError(value)
    }
}

impl From<tonic::Status> for GrpcError {
    fn from(value: tonic::Status) -> Self {
        GrpcError::Status(value)
    }
}

impl Display for GrpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GrpcError::TransportError(error) => write!(f, "GrpcError: {}", error),
            GrpcError::Status(status) => write!(f, "GrpcError: {}", status),
        }
    }
}

impl Error for GrpcError {}

impl Transport<KVCommand> for GrpcTransport {
    type Error = GrpcError;

    async fn append_entries(
        &self,
        peer_id: crate::raft::PeerId,
        msg: crate::raft::transport::AppendEntriesRequest<KVCommand>,
    ) -> Result<crate::raft::transport::AppendEntriesResponse, Self::Error> {
        let mut client = self.addr_map.get(&peer_id).unwrap().clone();
        let request = tonic::Request::new(msg.into());
        let response = client.append_entries(request).await?;
        Ok(response.into_inner().into())
    }

    async fn request_votes(
        &self,
        peer_id: crate::raft::PeerId,
        msg: crate::raft::transport::RequestVoteRequest,
    ) -> Result<crate::raft::transport::RequestVoteResponse, Self::Error> {
        let mut client = self.addr_map.get(&peer_id).unwrap().clone();
        let request = tonic::Request::new(msg.into());
        let response = client.request_vote(request).await?;
        Ok(response.into_inner().into())
    }
}
