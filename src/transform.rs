use crate::{
    kv::KVCommand,
    proto::{log_item::KvItem, AppendEntriesGrpcRequest, GetItem, LogItem, PutItem},
    raft::{transport::AppendEntriesRequest, LogEntry, PeerId, Term},
};

pub enum LogItemError {
    NoItem,
}

impl TryFrom<LogItem> for LogEntry<KVCommand> {
    type Error = LogItemError;
    fn try_from(value: LogItem) -> Result<Self, Self::Error> {
        let command = match value.kv_item {
            Some(kv_item) => match kv_item {
                KvItem::GetItem(get_item) => KVCommand::Get { key: get_item.key },
                KvItem::PutItem(put_item) => KVCommand::Put {
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
                KVCommand::Get { key } => KvItem::GetItem(GetItem { key }),
                KVCommand::Put { key, value } => KvItem::PutItem(PutItem { key, value }),
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
