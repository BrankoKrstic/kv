use futures::stream::{FuturesUnordered, StreamExt};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{
        mpsc::{self},
        oneshot,
    },
    time::{sleep, Duration},
};

const ELECTION_TIMEOUT_MIN: u64 = 150;
const ELECTION_TIMEOUT_MAX: u64 = 300;
const GET_VOTES_TIMEOUT_MIN: u64 = 700;
const GET_VOTES_TIMEOUT_MAX: u64 = 1200;
const HEARTBEAT_TIMEOUT: u64 = 10;

use crate::raft::transport::{RequestVoteRequest, RequestVoteResponse};

use super::{
    persist::{Persist, PersistedData},
    transport::{AppendEntriesRequest, AppendEntriesResponse, Transport},
    AppendError, LogEntry, Peer, PeerId, RaftCommand, Term, Topology,
};

#[derive(Debug, Clone, Copy)]
enum NodeState {
    Leader,
    Candidate,
    Follower,
}

enum VoteResult<LogCommand> {
    Winner,
    BiggerTerm(Term),
    TimedOut,
    Loser,
    OtherLeaderAppendRequest(
        AppendEntriesRequest<LogCommand>,
        oneshot::Sender<AppendEntriesResponse>,
    ),
}

pub struct Raft<T, LogCommand, P> {
    id: PeerId,
    term: Term,
    commit_index: Option<usize>,
    // last_applied: Option<usize>,
    state: NodeState,
    peers: Vec<Peer>,
    transport: T,
    voted_for: Option<PeerId>,
    log: Vec<LogEntry<LogCommand>>,
    event_receiver: mpsc::Receiver<RaftCommand<LogCommand>>,
    commit_channel: mpsc::Sender<LogCommand>,
    current_leader: Option<PeerId>,
    persist: P,
}

impl<T: Transport<LogCommand>, LogCommand: Clone + Send + Serialize, P: Persist<LogCommand>>
    Raft<T, LogCommand, P>
where
    LogCommand: for<'de> Deserialize<'de>,
{
    pub async fn new(
        transport: T,
        persist: P,
        topology: Topology,
        rx: mpsc::Receiver<RaftCommand<LogCommand>>,
        tx: mpsc::Sender<LogCommand>,
    ) -> Self {
        let id = topology.node_id;
        let peers = topology
            .peers
            .into_iter()
            .filter(|node| node.id != id)
            .collect();
        let PersistedData {
            term,
            voted_for,
            log,
        } = persist.load().await.unwrap().unwrap_or_default();
        Self {
            id,
            term,
            commit_index: None,
            // last_applied: None,
            state: NodeState::Follower,
            peers,
            transport,
            voted_for,
            log,
            event_receiver: rx,
            commit_channel: tx,
            current_leader: None,
            persist,
        }
    }
    async fn persist_state(&self) {
        let state = PersistedData {
            term: self.term,
            voted_for: self.voted_for,
            log: self.log.clone(),
        };
        if let Err(e) = self.persist.save(state).await {
            eprintln!("{}", e);
            panic!("Something went wrong");
        }
    }
    fn is_leader(&self) -> bool {
        matches!(self.state, NodeState::Leader)
    }
    fn submit_entry(&mut self, command: LogCommand) -> Result<(), AppendError> {
        if self.is_leader() {
            eprintln!(
                "Raft Instance {:?} submitting to log at index {}",
                self.id,
                self.log.len()
            );
            self.log.push(LogEntry::new(self.term, command));
            Ok(())
        } else {
            Err(AppendError::NotLeader(self.current_leader))
        }
    }
    async fn handle_command(&mut self, command: RaftCommand<LogCommand>) {
        match command {
            RaftCommand::AppendEntries { request, tx } => {
                let response = self.handle_append_entries(request).await;
                if response.success {
                    self.state = NodeState::Follower;
                }
                let _ = tx.send(response);
            }
            RaftCommand::RequestVote { request, tx } => {
                let response = self.handle_request_vote(request);
                self.persist_state().await;
                if response.vote_granted {
                    self.state = NodeState::Follower;
                }
                let _ = tx.send(response);
            }
            RaftCommand::SubmitEntry { request, tx } => {
                let result = self.submit_entry(request);
                self.persist_state().await;
                let _ = tx.send(result);
            }
        }
    }
    fn update_commit_index(&mut self, new_commit_index: Option<usize>) {
        if new_commit_index > self.commit_index {
            println!(
                "Raft Instance {:?} updating commit index {:?}",
                self.id, new_commit_index
            );
            let old_commit_index = self.commit_index;
            self.commit_index = new_commit_index;
            for item in &self.log
                [old_commit_index.unwrap_or_default()..new_commit_index.unwrap_or_default()]
            {
                // TODO: possibly switch this to a blocking channel
                let _ = self.commit_channel.try_send(item.command.clone());
            }
        }
    }
    async fn send_append_entries_helper(&self) -> Vec<(PeerId, usize, AppendEntriesResponse)> {
        let mut jhs = FuturesUnordered::new();

        for peer in &self.peers {
            let peer_id = peer.id;
            let prev_log_index = peer.log_index;
            let prev_log_term = prev_log_index.map(|i| self.log[i].term);
            let entries = self
                .log
                .iter()
                .skip(peer.log_index.map_or_else(|| 0, |v| v + 1))
                .cloned()
                .collect::<Vec<_>>();
            let entries_len = entries.len();

            let append_entries_request = AppendEntriesRequest {
                term: self.term,
                leader_id: self.id,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit: self.commit_index,
            };

            jhs.push(async move {
                let result = (self)
                    .transport
                    .append_entries(peer.id, append_entries_request)
                    .await;
                result
                    .map(|res| (peer_id, entries_len, res))
                    .map_err(|e| (peer_id, e))
            });
        }
        let mut out = vec![];

        while let Some(result) = jhs.next().await {
            match result {
                Ok(res) => out.push(res),
                Err(e) => eprintln!(
                    "Raft Instance {:?} failed to respond to append entries {}",
                    e.0, e.1
                ),
            };
        }

        out
    }
    async fn handle_append_entries(
        &mut self,
        req: AppendEntriesRequest<LogCommand>,
    ) -> AppendEntriesResponse {
        if self.term > req.term {
            return AppendEntriesResponse {
                term: self.term,
                success: false,
            };
        } else {
            self.current_leader = Some(req.leader_id);
            self.term = req.term;
        }
        let log_len = self.log.len();
        if let Some(prev_log_index) = req.prev_log_index {
            let prev_log_term = req.prev_log_term.unwrap();
            let prev_log = self.log.get(prev_log_index);
            if prev_log.is_none() || prev_log.unwrap().term != prev_log_term {
                self.persist_state().await;
                return AppendEntriesResponse {
                    term: self.term,
                    success: false,
                };
            }

            // TODO: Fix this awfulness
            for (i, log_entry) in req
                .entries
                .into_iter()
                .enumerate()
                .map(|(i, entry)| (i + prev_log_index + 1, entry))
            {
                if i < self.log.len() {
                    if self.log[i].term != log_entry.term {
                        self.log = self.log[0..i].to_vec();
                    }
                    self.log.push(log_entry);
                } else {
                    self.log.push(log_entry);
                }
            }
        } else {
            self.log = req.entries;
        }
        if req.leader_commit.is_none() || self.log.is_empty() {
            self.update_commit_index(None);
        } else {
            self.update_commit_index(Some(req.leader_commit.unwrap().min(self.log.len() - 1)));
        }
        if self.log.len() > log_len {
            eprintln!("Raft Instance {:?} appending logs", self.id);
        }
        self.persist_state().await;
        AppendEntriesResponse {
            term: self.term,
            success: true,
        }
    }
    async fn send_append_entries_requests(&mut self) {
        let results = self.send_append_entries_helper().await;

        for (peer_id, entries_len, result) in results {
            if result.success {
                let peer = self.peers.iter_mut().find(|p| p.id == peer_id);
                if let Some(peer) = peer {
                    if entries_len > 0 {
                        peer.log_index = Some(
                            peer.log_index
                                .map_or_else(|| entries_len - 1, |v| v + entries_len),
                        );
                    }
                }
            } else if result.term > self.term {
                self.term = result.term;
                self.current_leader = None;
                self.voted_for = None;
                self.state = NodeState::Follower;
                self.persist_state().await;
                return;
            } else {
                let peer = self.peers.iter_mut().find(|p| p.id == peer_id);
                // TODO: Actually have peers send their last log index here
                if let Some(peer) = peer {
                    peer.log_index =
                        peer.log_index
                            .and_then(|v| if v == 0 { None } else { Some(v - 1) });
                }
            }
        }
        let mut indices = self.peers.iter().map(|p| p.log_index).collect::<Vec<_>>();
        indices.sort_unstable();
        let new_commit_index = indices[(indices.len() - 1) / 2];
        self.update_commit_index(new_commit_index);
    }
    async fn get_votes(&mut self) -> VoteResult<LogCommand> {
        let cur_term = self.term;
        let needs_votes = (self.peers.len() + 1) / 2;
        let candidate_id = self.id;
        let last_log_index = if self.log.is_empty() {
            None
        } else {
            Some(self.log.len() - 1)
        };
        let last_log_term = last_log_index.map(|i| self.log[i].term);
        let peers = self.peers.clone();
        let mut jhs = peers
            .into_iter()
            .map(|peer| {
                self.transport.request_votes(
                    peer.id,
                    RequestVoteRequest {
                        candidate_id,
                        term: cur_term,
                        last_log_index,
                        last_log_term,
                    },
                )
            })
            .collect::<FuturesUnordered<_>>();

        let mut total_votes = 0;

        let timeout_duration = thread_rng().gen_range(GET_VOTES_TIMEOUT_MIN..GET_VOTES_TIMEOUT_MAX);
        let timeout = sleep(Duration::from_millis(timeout_duration));
        tokio::pin!(timeout);
        loop {
            tokio::select! {
                  _ = &mut timeout => {
                    eprintln!("Raft Instance {:?} vote timed out", candidate_id);
                    return VoteResult::TimedOut;
                  }
                  res = jhs.next() => {
                    if let Some(res) = res {
                        let res = match res {
                            Ok(res) => res,
                            Err(e) => {
                                eprintln!(
                                    "Raft Instance failed to respond to append entries {:?}",
                                    e
                                );
                                continue;
                            }
                        };
                      if res.vote_granted {
                        total_votes += 1;
                        if total_votes >= needs_votes {
                            eprintln!("Raft Instance {:?} won vote", candidate_id);

                            return VoteResult::Winner;
                        }
                      } else if res.term > self.term {
                          self.term = res.term;
                          return VoteResult::BiggerTerm(res.term);
                      }
                    } else {
                      return VoteResult::Loser;
                    }
                  },
                  command = self.event_receiver.recv() => {
                    match command.unwrap() {
                        RaftCommand::SubmitEntry { tx, .. } => {
                            let _ = tx.send(Err(AppendError::NotLeader(self.current_leader)));
                        },
                        RaftCommand::AppendEntries { request, tx } => {
                            if request.term >= self.term {
                                return VoteResult::OtherLeaderAppendRequest(request, tx);
                            } else {
                                let _ = tx.send(AppendEntriesResponse { term: self.term, success: false });
                            }
                        },
                        RaftCommand::RequestVote { request, tx } => {
                            if request.term > self.term  {
                                let _ = tx.send(RequestVoteResponse {
                                    term: self.term,
                                    vote_granted: true
                                });
                                eprintln!("Raft Instance {:?} voting for {:?}", self.id, request.candidate_id);
                                self.voted_for = Some(request.candidate_id);
                                self.state = NodeState::Follower;
                                self.term = request.term;
                                return VoteResult::BiggerTerm(request.term);
                            } else {
                                eprintln!("Raft Instance {:?} refusing vote for {:?}", self.id, request.candidate_id);
                                let _ = tx.send(RequestVoteResponse {
                                    term: self.term,
                                    vote_granted: false
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    pub async fn run(mut self) {
        loop {
            match self.state {
                NodeState::Leader => {
                    eprintln!("Raft Instance {:?} sending heartbeat", self.id);
                    self.send_append_entries_requests().await;
                    let timeout = Duration::from_millis(HEARTBEAT_TIMEOUT);
                    tokio::select! {
                            _ = sleep(timeout) => {},
                            msg = self.event_receiver.recv() => {
                                self.handle_command(msg.unwrap()).await;
                            }
                    }
                }
                NodeState::Candidate => {
                    self.term.0 += 1;
                    self.persist_state().await;
                    eprintln!(
                        "Raft Instance {:?} initiating vote term {:?}",
                        self.id, self.term
                    );
                    self.current_leader = None;
                    self.voted_for = Some(self.id);
                    let vote_result = self.get_votes().await;
                    match vote_result {
                        VoteResult::Winner => {
                            self.state = NodeState::Leader;
                        }
                        VoteResult::BiggerTerm(term) => {
                            self.term = term;
                            self.persist_state().await;
                            self.state = NodeState::Follower;
                        }
                        VoteResult::TimedOut => {
                            self.state = NodeState::Follower;
                        }
                        VoteResult::Loser => self.state = NodeState::Follower,
                        VoteResult::OtherLeaderAppendRequest(req, tx) => {
                            self.state = NodeState::Follower;
                            let _ = tx.send(self.handle_append_entries(req).await);
                        }
                    }
                }
                NodeState::Follower => {
                    let timeout_duration =
                        thread_rng().gen_range(ELECTION_TIMEOUT_MIN..ELECTION_TIMEOUT_MAX);
                    let timeout = Duration::from_millis(timeout_duration);
                    let timeout = sleep(timeout);
                    tokio::select! {
                        _ = timeout => {
                            self.state = NodeState::Candidate
                        }
                        _ = self.await_heartbeat() => {}
                    }
                }
            }
        }
    }
    async fn await_heartbeat(&mut self) {
        loop {
            let command = self.event_receiver.recv().await.unwrap();
            match command {
                RaftCommand::SubmitEntry { tx, .. } => {
                    let _ = tx.send(Err(AppendError::NotLeader(self.current_leader)));
                }
                RaftCommand::AppendEntries { request, tx } => {
                    let response = self.handle_append_entries(request).await;
                    let _ = tx.send(response);
                    return;
                }
                RaftCommand::RequestVote { request, tx } => {
                    let response = self.handle_request_vote(request);
                    self.persist_state().await;
                    let _ = tx.send(response);
                }
            }
        }
    }
    fn handle_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        match req.term {
            x if x < self.term => {
                eprintln!(
                    "Raft Instance {:?} refusing vote for {:?}",
                    self.id, req.candidate_id
                );
                return RequestVoteResponse {
                    term: self.term,
                    vote_granted: false,
                };
            }
            x if x > self.term => {
                self.current_leader = None;
                self.term = req.term;
                self.voted_for = None;
            }
            _ => {}
        }

        if self.voted_for == Some(req.candidate_id) {
            return RequestVoteResponse {
                term: self.term,
                vote_granted: true,
            };
        }
        let last_log_index = if self.log.is_empty() {
            None
        } else {
            Some(self.log.len() - 1)
        };
        let last_log_term = last_log_index.map(|i| self.log[i].term);

        if self.voted_for.is_none()
            && (req.last_log_term > last_log_term
                || (req.last_log_term == last_log_term && req.last_log_index >= last_log_index))
        {
            eprintln!(
                "Raft Instance {:?} voting for {:?}",
                self.id, req.candidate_id
            );
            self.voted_for = Some(req.candidate_id);
            RequestVoteResponse {
                term: self.term,
                vote_granted: true,
            }
        } else {
            eprintln!(
                "Raft Instance {:?} refusing vote for {:?}",
                self.id, req.candidate_id
            );
            RequestVoteResponse {
                term: self.term,
                vote_granted: false,
            }
        }
    }
}
