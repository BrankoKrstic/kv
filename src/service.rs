use tokio::sync::{mpsc, oneshot};

use crate::{
    kv::{EntryCommand, KVCommand},
    proto::{
        kv_server::Kv, AppendEntriesGrpcRequest, AppendEntriesGrpcResponse, RequestVoteGrpcRequest,
        RequestVoteGrpcResponse, SubmitEntryGrpcRequest, SubmitEntryGrpcResponse,
    },
    raft::RaftCommand,
    transform::{SubmitEntryErr, SubmitEntryRequest},
};

#[derive(Debug)]
pub struct KvService {
    raft_channel: mpsc::Sender<RaftCommand<KVCommand>>,
    kv_channel: mpsc::Sender<EntryCommand>,
}

impl KvService {
    pub fn new(
        raft_channel: mpsc::Sender<RaftCommand<KVCommand>>,
        kv_channel: mpsc::Sender<EntryCommand>,
    ) -> Self {
        Self {
            raft_channel,
            kv_channel,
        }
    }
}

#[tonic::async_trait]
impl Kv for KvService {
    async fn append_entries(
        &self,
        request: tonic::Request<AppendEntriesGrpcRequest>,
    ) -> Result<tonic::Response<AppendEntriesGrpcResponse>, tonic::Status> {
        let req = request
            .into_inner()
            .try_into()
            .map_err(|_| tonic::Status::new(tonic::Code::InvalidArgument, "Invalid log item"))?;

        let (tx, rx) = oneshot::channel();
        self.raft_channel
            .send(RaftCommand::AppendEntries { request: req, tx })
            .await
            .map_err(|_| tonic::Status::new(tonic::Code::Internal, "Failed to process request"))?;
        let res = tonic::Response::new(rx.await.unwrap().into());
        Ok(res)
    }

    async fn request_vote(
        &self,
        request: tonic::Request<RequestVoteGrpcRequest>,
    ) -> Result<tonic::Response<RequestVoteGrpcResponse>, tonic::Status> {
        println!("Got Request Vote Request");

        let req = request.into_inner().into();
        let (tx, rx) = oneshot::channel();
        self.raft_channel
            .send(RaftCommand::RequestVote { request: req, tx })
            .await
            .map_err(|_| tonic::Status::new(tonic::Code::Internal, "Failed to process request"))?;
        let res = tonic::Response::new(rx.await.unwrap().into());
        Ok(res)
    }
    async fn submit_entry(
        &self,
        request: tonic::Request<SubmitEntryGrpcRequest>,
    ) -> Result<tonic::Response<SubmitEntryGrpcResponse>, tonic::Status> {
        let req: Result<SubmitEntryRequest, _> = request.into_inner().try_into();
        let command = match req {
            Ok(r) => r.command,
            Err(_) => {
                return Ok(tonic::Response::new(
                    Err(SubmitEntryErr::InvalidCommand).into(),
                ))
            }
        };
        let (tx, rx) = oneshot::channel();
        self.kv_channel
            .send(EntryCommand { command, tx })
            .await
            .unwrap();
        let res = rx.await.unwrap();
        Ok(tonic::Response::new(res.into()))
    }
}
