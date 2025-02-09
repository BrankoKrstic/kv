use std::{collections::HashMap, error::Error, io, net::SocketAddr};

use kv::{
    proto::kv_client::KvClient,
    raft::PeerId,
    transform::{SubmitEntryRequest, SubmitEntryResponse},
};
use rand::seq::IteratorRandom;
use serde::Deserialize;
use tonic::codec::CompressionEncoding;
use ulid::Ulid;

#[tokio::main]
async fn main() {
    let config: Config = serde_json::from_str(&std::env::var("CONFIG").unwrap()).unwrap();
    let mut client = Client::new(config);
    let mut buf = String::new();
    loop {
        io::stdin().read_line(&mut buf).unwrap();
        match buf.split_once(' ') {
            Some(("GET", key)) => {
                let id = Ulid::new();
                let _ = client.get(id, key.trim()).await;
            }
            Some(("PUT", rest)) => 'ma: {
                let (key, val) = if rest.starts_with('"') {
                    if let Some((key, val)) = rest.split_once('\"') {
                        let val = val.trim();
                        (key, val)
                    } else {
                        eprintln!("Invalid command");
                        break 'ma;
                    }
                } else if let Some((key, val)) = rest.split_once(' ') {
                    (key, val)
                } else {
                    eprintln!("Invalid command");
                    break 'ma;
                };
                let id = Ulid::new();
                let _ = client.put(id, key.trim(), val.trim()).await;
            }
            _ => eprintln!("Invalid command"),
        }
        buf.clear();
    }
}

#[derive(Debug, Deserialize)]
struct Config {
    nodes: Vec<PeerInfo>,
}

#[derive(Debug, Deserialize)]
struct PeerInfo {
    id: PeerId,
    addr: SocketAddr,
}

struct Client {
    cur_leader: PeerId,
    servers: HashMap<PeerId, SocketAddr>,
}

impl Client {
    fn new(config: Config) -> Self {
        Self {
            cur_leader: config.nodes[0].id,
            servers: config.nodes.into_iter().map(|x| (x.id, x.addr)).collect(),
        }
    }
    async fn put(&mut self, id: Ulid, key: &str, value: &str) -> Result<(), Box<dyn Error>> {
        loop {
            let req = SubmitEntryRequest {
                command: kv::kv::KVCommand::Put {
                    id,
                    key: key.to_string(),
                    value: value.to_string(),
                },
            };
            let addr = self.servers.get(&self.cur_leader).unwrap();
            let request = tonic::Request::new(req.clone().into());

            if let Ok(client) = KvClient::connect(format!("http://{}", addr)).await {
                let mut client = client
                    .accept_compressed(CompressionEncoding::Gzip)
                    .send_compressed(CompressionEncoding::Gzip);

                let response = client.submit_entry(request).await;
                let response = response?.into_inner();
                match response.try_into() {
                    Ok(SubmitEntryResponse { .. }) => break,
                    Err(e) => match e {
                        kv::transform::SubmitEntryErr::DuplicateRequest(_) => break,
                        kv::transform::SubmitEntryErr::NotLeader => {
                            let new_leader = self
                                .servers
                                .iter()
                                .filter(|l| l.0 != &self.cur_leader)
                                .choose(&mut rand::thread_rng())
                                .unwrap();
                            self.cur_leader = *new_leader.0;
                        }
                        kv::transform::SubmitEntryErr::InvalidCommand => {
                            eprintln!("Failed to process command!");
                            Err(e)?;
                        }
                    },
                }
            } else {
                let new_leader = self
                    .servers
                    .iter()
                    .filter(|l| l.0 != &self.cur_leader)
                    .choose(&mut rand::thread_rng())
                    .unwrap();
                self.cur_leader = *new_leader.0;
            }
        }
        Ok(())
    }
    async fn get(&mut self, id: Ulid, key: &str) -> Result<(), Box<dyn Error>> {
        loop {
            let addr = self.servers.get(&self.cur_leader).unwrap();
            let req = SubmitEntryRequest {
                command: kv::kv::KVCommand::Get {
                    id,
                    key: key.to_string(),
                },
            };
            let request = tonic::Request::new(req.into());
            match KvClient::connect(format!("http://{}", addr)).await {
                Ok(client) => {
                    let mut client = client
                        .accept_compressed(CompressionEncoding::Gzip)
                        .send_compressed(CompressionEncoding::Gzip);
                    let response = client.submit_entry(request).await;
                    let response = response?.into_inner();
                    match response.try_into() {
                        Ok(SubmitEntryResponse { value }) => {
                            match value {
                                Some(val) => {
                                    eprintln!("{}: {}", key, val);
                                }
                                None => {
                                    eprintln!("{}: empty", key);
                                }
                            }
                            break;
                        }
                        Err(e) => match e {
                            kv::transform::SubmitEntryErr::DuplicateRequest(val) => {
                                match val {
                                    Some(val) => println!("{}: {}", key, val),
                                    None => println!("{}: empty", key),
                                }
                                break;
                            }
                            kv::transform::SubmitEntryErr::NotLeader => {
                                let new_leader = self
                                    .servers
                                    .iter()
                                    .filter(|l| l.0 != &self.cur_leader)
                                    .choose(&mut rand::thread_rng())
                                    .unwrap();
                                self.cur_leader = *new_leader.0;
                            }
                            kv::transform::SubmitEntryErr::InvalidCommand => {
                                eprintln!("Failed to process command!");
                                Err(e)?;
                            }
                        },
                    }
                }
                Err(_) => {
                    let new_leader = self
                        .servers
                        .iter()
                        .filter(|l| l.0 != &self.cur_leader)
                        .choose(&mut rand::thread_rng())
                        .unwrap();
                    self.cur_leader = *new_leader.0;
                }
            }
        }
        Ok(())
    }
}
