use ulid::Ulid;

use crate::{
    kv::KVCommand,
    proto::{
        log_item::KvItem, submit_entry_grpc_request::EntryItem, AppendEntriesGrpcRequest,
        AppendEntriesGrpcResponse, GetItem, LogItem, PutItem, RequestVoteGrpcRequest,
        RequestVoteGrpcResponse, SubmitEntryGrpcRequest, SubmitEntryGrpcResponse,
    },
    raft::{
        transport::{
            AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
        },
        LogEntry, PeerId, Term,
    },
};

pub enum LogItemError {
    NoItem,
    InvalidUlid,
}

impl TryFrom<LogItem> for LogEntry<KVCommand> {
    type Error = LogItemError;
    fn try_from(value: LogItem) -> Result<Self, Self::Error> {
        let command = match value.kv_item {
            Some(kv_item) => match kv_item {
                KvItem::GetItem(get_item) => KVCommand::Get {
                    id: Ulid::from_string(&get_item.id[..])
                        .map_err(|_| LogItemError::InvalidUlid)?,
                    key: get_item.key,
                },
                KvItem::PutItem(put_item) => KVCommand::Put {
                    id: Ulid::from_string(&put_item.id[..])
                        .map_err(|_| LogItemError::InvalidUlid)?,
                    key: put_item.key,
                    value: put_item.value,
                },
            },
            None => return Err(LogItemError::NoItem),
        };
        Ok(LogEntry {
            term: Term(value.term),
            command,
        })
    }
}

impl From<LogEntry<KVCommand>> for LogItem {
    fn from(value: LogEntry<KVCommand>) -> Self {
        LogItem {
            term: value.term.0,
            kv_item: Some(match value.command {
                KVCommand::Get { id, key } => KvItem::GetItem(GetItem {
                    id: id.to_string(),
                    key,
                }),
                KVCommand::Put { id, key, value } => KvItem::PutItem(PutItem {
                    id: id.to_string(),
                    key,
                    value,
                }),
            }),
        }
    }
}

impl TryFrom<AppendEntriesGrpcRequest> for AppendEntriesRequest<KVCommand> {
    type Error = LogItemError;

    fn try_from(value: AppendEntriesGrpcRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            term: Term(value.term),
            leader_id: PeerId(value.leader_id),
            prev_log_index: value.prev_log_index.map(|x| x as usize),
            prev_log_term: value.prev_log_term.map(Term),
            entries: value
                .entries
                .into_iter()
                .map(|v| v.try_into())
                .collect::<Result<Vec<_>, _>>()?,
            leader_commit: value.leader_commit.map(|x| x as usize),
        })
    }
}

impl From<AppendEntriesRequest<KVCommand>> for AppendEntriesGrpcRequest {
    fn from(value: AppendEntriesRequest<KVCommand>) -> Self {
        Self {
            term: value.term.0,
            leader_id: value.leader_id.0,
            prev_log_index: value.prev_log_index.map(|x| x as u64),
            prev_log_term: value.prev_log_term.map(|x| x.0),
            entries: value.entries.into_iter().map(|x| x.into()).collect(),
            leader_commit: value.leader_commit.map(|x| x as u64),
        }
    }
}

impl From<AppendEntriesGrpcResponse> for AppendEntriesResponse {
    fn from(value: AppendEntriesGrpcResponse) -> Self {
        Self {
            term: Term(value.term),
            success: value.success,
        }
    }
}

impl From<AppendEntriesResponse> for AppendEntriesGrpcResponse {
    fn from(value: AppendEntriesResponse) -> Self {
        Self {
            term: value.term.0,
            success: value.success,
        }
    }
}

impl From<RequestVoteGrpcRequest> for RequestVoteRequest {
    fn from(value: RequestVoteGrpcRequest) -> Self {
        Self {
            candidate_id: PeerId(value.candidate_id),
            term: Term(value.term),
            last_log_index: value.last_log_index.map(|x| x as usize),
            last_log_term: value.last_log_term.map(Term),
        }
    }
}
impl From<RequestVoteRequest> for RequestVoteGrpcRequest {
    fn from(value: RequestVoteRequest) -> Self {
        Self {
            candidate_id: value.candidate_id.0,
            term: value.term.0,
            last_log_index: value.last_log_index.map(|x| x as u64),
            last_log_term: value.last_log_term.map(|x| x.0),
        }
    }
}

impl From<RequestVoteGrpcResponse> for RequestVoteResponse {
    fn from(value: RequestVoteGrpcResponse) -> Self {
        Self {
            term: Term(value.term),
            vote_granted: value.vote_granted,
        }
    }
}
impl From<RequestVoteResponse> for RequestVoteGrpcResponse {
    fn from(value: RequestVoteResponse) -> Self {
        Self {
            term: value.term.0,
            vote_granted: value.vote_granted,
        }
    }
}

pub struct SubmitEntryRequest {
    pub command: KVCommand,
}

pub struct SubmitEntryResponse {
    pub value: Option<String>,
}

pub enum SubmitEntryErr {
    DuplicateRequest,
    NotLeader,
    InvalidCommand,
}

impl TryFrom<SubmitEntryGrpcRequest> for SubmitEntryRequest {
    type Error = LogItemError;
    fn try_from(value: SubmitEntryGrpcRequest) -> Result<Self, Self::Error> {
        let item = match value.entry_item {
            Some(entry_item) => match entry_item {
                EntryItem::GetItem(get_item) => KVCommand::Get {
                    id: Ulid::from_string(&get_item.id[..])
                        .map_err(|_| LogItemError::InvalidUlid)?,
                    key: get_item.key,
                },
                EntryItem::PutItem(put_item) => KVCommand::Put {
                    id: Ulid::from_string(&put_item.id[..])
                        .map_err(|_| LogItemError::InvalidUlid)?,
                    key: put_item.key,
                    value: put_item.value,
                },
            },
            None => return Err(LogItemError::NoItem),
        };
        Ok(SubmitEntryRequest { command: item })
    }
}

impl From<SubmitEntryRequest> for SubmitEntryGrpcRequest {
    fn from(value: SubmitEntryRequest) -> Self {
        SubmitEntryGrpcRequest {
            entry_item: Some(match value.command {
                KVCommand::Get { id, key } => EntryItem::GetItem(GetItem {
                    key,
                    id: id.to_string(),
                }),
                KVCommand::Put { id, key, value } => EntryItem::PutItem(PutItem {
                    key,
                    value,
                    id: id.to_string(),
                }),
            }),
        }
    }
}

impl From<Result<SubmitEntryResponse, SubmitEntryErr>> for SubmitEntryGrpcResponse {
    fn from(value: Result<SubmitEntryResponse, SubmitEntryErr>) -> Self {
        match value {
            Ok(res) => SubmitEntryGrpcResponse {
                value: res.value,
                duplicate: false,
                not_leader: false,
                invalid_command: false,
            },
            Err(e) => match e {
                SubmitEntryErr::DuplicateRequest => SubmitEntryGrpcResponse {
                    value: None,
                    duplicate: true,
                    not_leader: false,
                    invalid_command: false,
                },
                SubmitEntryErr::NotLeader => SubmitEntryGrpcResponse {
                    value: None,
                    duplicate: false,
                    not_leader: true,
                    invalid_command: false,
                },
                SubmitEntryErr::InvalidCommand => SubmitEntryGrpcResponse {
                    value: None,
                    duplicate: false,
                    not_leader: false,
                    invalid_command: true,
                },
            },
        }
    }
}

impl TryFrom<SubmitEntryGrpcResponse> for SubmitEntryResponse {
    type Error = SubmitEntryErr;

    fn try_from(value: SubmitEntryGrpcResponse) -> Result<Self, Self::Error> {
        if value.duplicate {
            Err(SubmitEntryErr::DuplicateRequest)
        } else if value.invalid_command {
            Err(SubmitEntryErr::InvalidCommand)
        } else if value.not_leader {
            Err(SubmitEntryErr::NotLeader)
        } else {
            Ok(SubmitEntryResponse { value: value.value })
        }
    }
}
