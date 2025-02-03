use super::{LogEntry, PeerId, Term};

pub trait Transport<LogCommand> {
    type Error: std::error::Error;
    #[allow(async_fn_in_trait)]
    async fn append_entries(
        &self,
        peer_id: PeerId,
        msg: AppendEntriesRequest<LogCommand>,
    ) -> Result<AppendEntriesResponse, Self::Error>;
    #[allow(async_fn_in_trait)]
    async fn request_votes(
        &self,
        peer_id: PeerId,
        msg: RequestVoteRequest,
    ) -> Result<RequestVoteResponse, Self::Error>;
}

pub struct AppendEntriesResponse {
    pub(crate) term: Term,
    pub(crate) success: bool,
}
pub struct RequestVoteRequest {
    pub(crate) candidate_id: PeerId,
    pub(crate) term: Term,
    pub(crate) last_log_index: Option<usize>,
    pub(crate) last_log_term: Option<Term>,
}
pub struct RequestVoteResponse {
    pub(crate) term: Term,
    pub(crate) vote_granted: bool,
}

pub struct AppendEntriesRequest<LogCommand> {
    pub(crate) term: Term,
    pub(crate) leader_id: PeerId,
    pub(crate) prev_log_index: Option<usize>,
    pub(crate) prev_log_term: Option<Term>,
    pub(crate) entries: Vec<LogEntry<LogCommand>>,
    pub(crate) leader_commit: Option<usize>,
}
