//! Terminal SSH interaktif (PTY). Data terminal tidak pernah dicatat.
use crate::error::Result;
use crate::ssh::Conn;
use base64::Engine;
use serde_json::json;
use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub enum TermMsg {
    Input(Vec<u8>),
    Resize(u32, u32),
    Close,
}

pub struct TerminalHandle {
    pub tx: Sender<TermMsg>,
    pub server_id: i64,
}

pub fn open(app: AppHandle, state: Arc<crate::state::AppState>, conn: &Conn, id: String, cols: u32, rows: u32) -> Result<TerminalHandle> {
    let sess = conn.new_session()?;
    let mut ch = sess.channel_session()?;
    ch.request_pty("xterm-256color", None, Some((cols.max(20), rows.max(5), 0, 0)))?;
    ch.shell()?;
    sess.set_blocking(false);
    let (tx, rx) = mpsc::channel::<TermMsg>();
    let server_id = conn.profile.id;
    let tid = id.clone();
    std::thread::spawn(move || {
        let _keep = sess; // sesi harus hidup selama channel dipakai
        let b64 = base64::engine::general_purpose::STANDARD;
        let mut buf = [0u8; 16 * 1024];
        'outer: loop {
            let mut idle = true;
            match ch.read(&mut buf) {
                Ok(0) => {
                    if ch.eof() {
                        break;
                    }
                }
                Ok(n) => {
                    idle = false;
                    let _ = app.emit("terminal-output", json!({ "id": tid, "data": b64.encode(&buf[..n]) }));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(_) => break,
            }
            loop {
                match rx.try_recv() {
                    Ok(TermMsg::Input(data)) => {
                        idle = false;
                        let mut off = 0;
                        let mut spins = 0;
                        while off < data.len() {
                            match ch.write(&data[off..]) {
                                Ok(n) => off += n,
                                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                    spins += 1;
                                    if spins > 2000 {
                                        break 'outer;
                                    }
                                    std::thread::sleep(Duration::from_millis(2));
                                }
                                Err(_) => break 'outer,
                            }
                        }
                        let _ = ch.flush();
                    }
                    Ok(TermMsg::Resize(c, r)) => {
                        let _ = ch.request_pty_size(c.max(20), r.max(5), None, None);
                    }
                    Ok(TermMsg::Close) | Err(TryRecvError::Disconnected) => {
                        let _ = ch.close();
                        break 'outer;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if idle {
                std::thread::sleep(Duration::from_millis(8));
            }
        }
        let _ = ch.close();
        state.terminals.lock().unwrap_or_else(|e| e.into_inner()).remove(&tid);
        let _ = app.emit("terminal-closed", json!({ "id": tid }));
    });
    Ok(TerminalHandle { tx, server_id })
}
