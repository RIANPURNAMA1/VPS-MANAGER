//! File Manager berbasis SFTP. Semua path divalidasi (absolut, tanpa `..`).
use crate::error::{AppError, Result};
use crate::security::validate;
use crate::ssh::Conn;
use serde::Serialize;
use ssh2::{FileStat, OpenFlags, OpenType, Sftp};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

pub const MAX_TEXT_BYTES: u64 = 512 * 1024;

#[derive(Serialize, Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub mtime: Option<i64>,
    pub perm: String,
    pub perm_octal: String,
    pub uid: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<FileEntry>,
}

pub fn perm_string(mode: u32, is_dir: bool, is_link: bool) -> String {
    let t = if is_link { 'l' } else if is_dir { 'd' } else { '-' };
    let mut s = String::from(t);
    for shift in [6, 3, 0] {
        let b = (mode >> shift) & 7;
        s.push(if b & 4 != 0 { 'r' } else { '-' });
        s.push(if b & 2 != 0 { 'w' } else { '-' });
        s.push(if b & 1 != 0 { 'x' } else { '-' });
    }
    s
}

fn entry(name: String, path: String, st: &FileStat) -> FileEntry {
    let mode = st.perm.unwrap_or(0);
    let is_link = mode & 0o170000 == 0o120000;
    FileEntry {
        name,
        path,
        is_dir: st.is_dir(),
        is_symlink: is_link,
        size: st.size.unwrap_or(0),
        mtime: st.mtime.map(|t| t as i64),
        perm: perm_string(mode, st.is_dir(), is_link),
        perm_octal: format!("{:04o}", mode & 0o7777),
        uid: st.uid,
    }
}

pub fn list(conn: &Conn, path: &str) -> Result<Listing> {
    let path = validate::remote_path(path)?;
    let entries = conn.with_sftp(|sftp| {
        let mut v: Vec<FileEntry> = sftp
            .readdir(Path::new(&path))?
            .into_iter()
            .filter_map(|(p, st)| {
                let name = p.file_name()?.to_string_lossy().into_owned();
                if name == "." || name == ".." {
                    return None;
                }
                let full = validate::join_remote(&path, &name).ok()?;
                Some(entry(name, full, &st))
            })
            .collect();
        v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(v)
    })?;
    Ok(Listing { parent: (path != "/").then(|| validate::parent_of(&path)), path, entries })
}

fn exists(sftp: &Sftp, p: &str) -> bool {
    sftp.lstat(Path::new(p)).is_ok()
}

pub fn mkdir(conn: &Conn, dir: &str, name: &str) -> Result<String> {
    let target = validate::join_remote(&validate::remote_path(dir)?, name)?;
    conn.with_sftp(|s| {
        if exists(s, &target) {
            return Err(AppError::new("exists", "Nama tersebut sudah ada."));
        }
        s.mkdir(Path::new(&target), 0o755)?;
        Ok(())
    })?;
    Ok(target)
}

pub fn rename(conn: &Conn, from: &str, new_name: &str) -> Result<String> {
    let from = validate::remote_path(from)?;
    if validate::is_protected(&from) {
        return Err(AppError::denied("Path sistem dilindungi dan tidak dapat diubah namanya."));
    }
    let to = validate::join_remote(&validate::parent_of(&from), new_name)?;
    conn.with_sftp(|s| {
        if exists(s, &to) {
            return Err(AppError::new("exists", "Sudah ada item dengan nama tersebut."));
        }
        s.rename(Path::new(&from), Path::new(&to), None)?;
        Ok(())
    })?;
    Ok(to)
}

/// Hapus file atau direktori KOSONG. Konfirmasi wajib dari pemanggil.
pub fn delete(conn: &Conn, path: &str, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(AppError::denied("Penghapusan memerlukan konfirmasi."));
    }
    let path = validate::remote_path(path)?;
    if validate::is_protected(&path) {
        return Err(AppError::denied("Path sistem dilindungi dan tidak dapat dihapus."));
    }
    conn.with_sftp(|s| {
        let st = s.lstat(Path::new(&path))?;
        if st.is_dir() {
            s.rmdir(Path::new(&path)).map_err(|e| {
                let ae: AppError = e.into();
                if ae.code == "ssh" {
                    AppError::new("not_empty", "Direktori tidak kosong atau tidak bisa dihapus. Hapus isinya terlebih dahulu.")
                } else {
                    ae
                }
            })
        } else {
            Ok(s.unlink(Path::new(&path))?)
        }
    })
}

pub fn read_text(conn: &Conn, path: &str) -> Result<String> {
    let path = validate::remote_path(path)?;
    conn.with_sftp(|s| {
        let st = s.stat(Path::new(&path))?;
        if st.is_dir() {
            return Err(AppError::invalid("Ini adalah direktori."));
        }
        let size = st.size.unwrap_or(0);
        if size > MAX_TEXT_BYTES {
            return Err(AppError::new("too_large", format!("File {} KB melebihi batas edit {} KB.", size / 1024, MAX_TEXT_BYTES / 1024)));
        }
        let mut f = s.open(Path::new(&path))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        String::from_utf8(buf).map_err(|_| AppError::new("binary", "File bukan teks UTF-8 sehingga tidak dapat diedit."))
    })
}

pub fn write_text(conn: &Conn, path: &str, content: &str, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(AppError::denied("Menyimpan perubahan file memerlukan konfirmasi."));
    }
    let path = validate::remote_path(path)?;
    if content.len() as u64 > MAX_TEXT_BYTES {
        return Err(AppError::new("too_large", "Isi file melebihi batas edit."));
    }
    conn.with_sftp(|s| {
        let st = s.stat(Path::new(&path))?;
        let mode = (st.perm.unwrap_or(0o644) & 0o7777) as i32;
        let tmp = format!("{path}.vpsm-tmp");
        {
            let mut f = s.open_mode(Path::new(&tmp), OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE, mode, OpenType::File)?;
            f.write_all(content.as_bytes())?;
            f.flush()?;
        }
        s.rename(Path::new(&tmp), Path::new(&path), Some(ssh2::RenameFlags::OVERWRITE | ssh2::RenameFlags::ATOMIC | ssh2::RenameFlags::NATIVE))
            .map_err(AppError::from)?;
        Ok(())
    })
}

#[derive(Serialize, Clone)]
struct Progress<'a> {
    id: &'a str,
    name: &'a str,
    direction: &'a str,
    done: u64,
    total: u64,
    finished: bool,
}

fn emit(app: &AppHandle, id: &str, name: &str, dir: &str, done: u64, total: u64, finished: bool) {
    let _ = app.emit("transfer-progress", Progress { id, name, direction: dir, done, total, finished });
}

fn unique_local(dir: &Path, name: &str) -> PathBuf {
    let mut p = dir.join(name);
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    let mut i = 1;
    while p.exists() {
        p = dir.join(format!("{stem} ({i}){ext}"));
        i += 1;
    }
    p
}

pub fn download(app: &AppHandle, conn: &Conn, remote: &str, dest_dir: &str, max_bytes: u64, transfer_id: &str) -> Result<String> {
    let remote = validate::remote_path(remote)?;
    let name = validate::sanitize_filename(&remote)?;
    let dir = PathBuf::from(dest_dir);
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir)?;
    }
    let sess = conn.new_session()?;
    let sftp = sess.sftp()?;
    let st = sftp.stat(Path::new(&remote))?;
    if st.is_dir() {
        return Err(AppError::invalid("Unduh direktori belum didukung. Pilih file."));
    }
    let total = st.size.unwrap_or(0);
    if total > max_bytes {
        return Err(AppError::new("too_large", format!("Ukuran file {} MB melebihi batas transfer {} MB (Pengaturan).", total / 1_048_576, max_bytes / 1_048_576)));
    }
    let target = unique_local(&dir, &name);
    let part = target.with_extension("vpsm-part");
    let result = (|| -> Result<()> {
        let mut src = sftp.open(Path::new(&remote))?;
        let mut out = File::create(&part)?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        let mut last = std::time::Instant::now();
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            if last.elapsed().as_millis() > 120 {
                emit(app, transfer_id, &name, "download", done, total, false);
                last = std::time::Instant::now();
            }
        }
        out.flush()?;
        if total > 0 && done != total {
            return Err(AppError::new("incomplete", "Ukuran file hasil unduhan tidak sama dengan ukuran di server."));
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            std::fs::rename(&part, &target)?;
            emit(app, transfer_id, &name, "download", total, total, true);
            Ok(target.to_string_lossy().into_owned())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

pub fn upload(app: &AppHandle, conn: &Conn, local: &str, remote_dir: &str, overwrite: bool, max_bytes: u64, transfer_id: &str) -> Result<String> {
    let local_path = PathBuf::from(local);
    let meta = std::fs::metadata(&local_path).map_err(|_| AppError::new("not_found", "File lokal tidak ditemukan."))?;
    if !meta.is_file() {
        return Err(AppError::invalid("Pilih sebuah file (bukan folder)."));
    }
    let total = meta.len();
    if total > max_bytes {
        return Err(AppError::new("too_large", format!("Ukuran file {} MB melebihi batas transfer {} MB (Pengaturan).", total / 1_048_576, max_bytes / 1_048_576)));
    }
    let name = validate::sanitize_filename(&local_path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())?;
    let target = validate::join_remote(&validate::remote_path(remote_dir)?, &name)?;
    let tmp = format!("{target}.vpsm-part");

    let sess = conn.new_session()?;
    let sftp = sess.sftp()?;
    if exists(&sftp, &target) && !overwrite {
        return Err(AppError::new("exists", format!("{name} sudah ada di server. Konfirmasi untuk menimpa.")));
    }
    let res = (|| -> Result<()> {
        let mut src = File::open(&local_path)?;
        let mut dst = sftp.open_mode(Path::new(&tmp), OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE, 0o644, OpenType::File)?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        let mut last = std::time::Instant::now();
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n])?;
            done += n as u64;
            if last.elapsed().as_millis() > 120 {
                emit(app, transfer_id, &name, "upload", done, total, false);
                last = std::time::Instant::now();
            }
        }
        dst.flush()?;
        drop(dst);
        let remote_size = sftp.stat(Path::new(&tmp))?.size.unwrap_or(0);
        if remote_size != total {
            return Err(AppError::new("incomplete", "Ukuran file di server tidak sama dengan file lokal; unggahan dibatalkan."));
        }
        sftp.rename(Path::new(&tmp), Path::new(&target), Some(ssh2::RenameFlags::OVERWRITE | ssh2::RenameFlags::ATOMIC | ssh2::RenameFlags::NATIVE))?;
        Ok(())
    })();
    match res {
        Ok(()) => {
            emit(app, transfer_id, &name, "upload", total, total, true);
            Ok(target)
        }
        Err(e) => {
            let _ = sftp.unlink(Path::new(&tmp));
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_permissions() {
        assert_eq!(perm_string(0o755, true, false), "drwxr-xr-x");
        assert_eq!(perm_string(0o644, false, false), "-rw-r--r--");
        assert_eq!(perm_string(0o777, false, true), "lrwxrwxrwx");
    }

    #[test]
    fn unique_local_names() {
        let dir = std::env::temp_dir().join(format!("vpsm-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        assert_eq!(unique_local(&dir, "a.txt").file_name().unwrap(), "a (1).txt");
        assert_eq!(unique_local(&dir, "b.txt").file_name().unwrap(), "b.txt");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
