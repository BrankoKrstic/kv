use std::{error::Error, marker::PhantomData, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
};

use super::{LogEntry, PeerId, Term};

#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedData<LogCommand> {
    pub term: Term,
    pub voted_for: Option<PeerId>,
    pub log: Vec<LogEntry<LogCommand>>,
}

impl<LogCommand> Default for PersistedData<LogCommand> {
    fn default() -> Self {
        Self {
            term: Term(0),
            voted_for: None,
            log: vec![],
        }
    }
}

pub trait Persist<LogCommand> {
    async fn save(&self, data: PersistedData<LogCommand>) -> Result<(), Box<dyn Error>>;
    async fn load(&self) -> Result<Option<PersistedData<LogCommand>>, Box<dyn Error>>;
}

pub struct DiskPersist<LogCommand> {
    path: PathBuf,
    _boo: PhantomData<LogCommand>,
}

impl<LogCommand> Persist<LogCommand> for DiskPersist<LogCommand>
where
    LogCommand: for<'de> Deserialize<'de> + Serialize,
{
    async fn save(&self, data: PersistedData<LogCommand>) -> Result<(), Box<dyn Error>> {
        let fp = File::create(&self.path).await?;
        let mut writer = BufWriter::new(fp);
        let serialized = bincode::serialize(&data)?;
        writer.write_all(&serialized[..]).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn load(&self) -> Result<Option<PersistedData<LogCommand>>, Box<dyn Error>> {
        let fp = match File::open(&self.path).await {
            Ok(fp) => fp,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => return Ok(None),
                _ => {
                    return Err(e)?;
                }
            },
        };

        let mut reader = BufReader::new(fp);
        let mut buf = vec![];
        reader.read_to_end(&mut buf).await?;
        let out = bincode::deserialize(&buf[..])?;
        Ok(Some(out))
    }
}

impl<LogCommand> DiskPersist<LogCommand> {
    pub fn new<T: Into<PathBuf>>(path: T) -> Self {
        Self {
            path: path.into(),
            _boo: PhantomData,
        }
    }
}
