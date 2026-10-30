use crate::ssh::Conn;
use crate::storage::Db;
use crate::terminal::TerminalHandle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub db: Db,
    pub data_dir: PathBuf,
    pub conns: Mutex<HashMap<i64, Arc<Conn>>>,
    pub terminals: Mutex<HashMap<String, TerminalHandle>>,
}

impl AppState {
    pub fn new(db: Db, data_dir: PathBuf) -> Self {
        Self { db, data_dir, conns: Mutex::new(HashMap::new()), terminals: Mutex::new(HashMap::new()) }
    }

    pub fn conn(&self, id: i64) -> crate::error::Result<Arc<Conn>> {
        self.conns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .cloned()
            .ok_or_else(crate::error::AppError::not_connected)
    }
}

pub type Shared = Arc<AppState>;
