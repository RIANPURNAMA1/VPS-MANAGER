//! Deployment Manager berbasis Git. Aturan keamanan: tidak ada `git reset --hard`, tidak menimpa
//! perubahan lokal (deploy dibatalkan jika working tree kotor), migrasi hanya dengan konfirmasi.
use crate::error::{AppError, Result};
use crate::security::risk::{self, Risk};
use crate::security::validate::{self, shell_quote};
use crate::ssh::Conn;
use crate::storage::Db;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Project {
    pub id: i64,
    pub server_id: i64,
    pub name: String,
    pub path: String,
    pub repo_url: Option<String>,
    pub branch: String,
    pub build_steps: Vec<String>,
    pub restart_service: Option<String>,
    pub verify_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProjectInput {
    pub id: Option<i64>,
    pub server_id: i64,
    pub name: String,
    pub path: String,
    pub repo_url: Option<String>,
    pub branch: String,
    pub build_steps: Vec<String>,
    pub restart_service: Option<String>,
    pub verify_url: Option<String>,
}

impl ProjectInput {
    pub fn validate(&self) -> Result<ProjectInput> {
        let mut p = self.clone();
        if p.name.trim().is_empty() {
            return Err(AppError::invalid("Nama proyek wajib diisi."));
        }
        p.path = validate::remote_path(&p.path)?;
        if validate::is_protected(&p.path) {
            return Err(AppError::invalid("Path proyek tidak boleh direktori sistem."));
        }
        p.branch = validate::branch(p.branch.trim())?;
        p.build_steps = p.build_steps.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        for s in &p.build_steps {
            if risk::classify(s).risk == Risk::Blocked {
                return Err(AppError::invalid(format!("Langkah build diblokir karena berbahaya: {s}")));
            }
        }
        p.restart_service = p.restart_service.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(validate::unit_name).transpose()?;
        p.verify_url = p.verify_url.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(validate::http_url).transpose()?;
        p.repo_url = p.repo_url.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Ok(p)
    }
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Project> {
    let steps: String = r.get(6)?;
    Ok(Project {
        id: r.get(0)?,
        server_id: r.get(1)?,
        name: r.get(2)?,
        path: r.get(3)?,
        repo_url: r.get(4)?,
        branch: r.get(5)?,
        build_steps: serde_json::from_str(&steps).unwrap_or_default(),
        restart_service: r.get(7)?,
        verify_url: r.get(8)?,
    })
}
const PCOLS: &str = "id,server_id,name,path,repo_url,branch,build_steps,restart_service,verify_url";

pub fn projects(db: &Db, server_id: i64) -> Result<Vec<Project>> {
    let c = db.lock();
    let mut s = c.prepare(&format!("SELECT {PCOLS} FROM deploy_projects WHERE server_id=?1 ORDER BY name"))?;
    let v = s.query_map([server_id], row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

pub fn project(db: &Db, id: i64) -> Result<Project> {
    db.lock()
        .query_row(&format!("SELECT {PCOLS} FROM deploy_projects WHERE id=?1"), [id], row)
        .map_err(|_| AppError::new("not_found", "Proyek deployment tidak ditemukan."))
}

pub fn save_project(db: &Db, input: &ProjectInput) -> Result<Project> {
    let p = input.validate()?;
    let steps = serde_json::to_string(&p.build_steps)?;
    let id = {
        let c = db.lock();
        match p.id {
            Some(id) => {
                c.execute(
                    "UPDATE deploy_projects SET name=?1,path=?2,repo_url=?3,branch=?4,build_steps=?5,restart_service=?6,verify_url=?7 WHERE id=?8",
                    params![p.name.trim(), p.path, p.repo_url, p.branch, steps, p.restart_service, p.verify_url, id],
                )?;
                id
            }
            None => {
                c.execute(
                    "INSERT INTO deploy_projects (server_id,name,path,repo_url,branch,build_steps,restart_service,verify_url) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![p.server_id, p.name.trim(), p.path, p.repo_url, p.branch, steps, p.restart_service, p.verify_url],
                )?;
                c.last_insert_rowid()
            }
        }
    };
    project(db, id)
}

pub fn delete_project(db: &Db, id: i64) -> Result<()> {
    db.lock().execute("DELETE FROM deploy_projects WHERE id=?1", [id])?;
    Ok(())
}

#[derive(Serialize, Debug, Clone)]
pub struct Deployment {
    pub id: i64,
    pub project_id: i64,
    pub kind: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub from_commit: Option<String>,
    pub to_commit: Option<String>,
    pub log: String,
}

pub fn history(db: &Db, project_id: i64) -> Result<Vec<Deployment>> {
    let c = db.lock();
    let mut s = c.prepare(
        "SELECT id,project_id,kind,started_at,finished_at,status,from_commit,to_commit,log FROM deployments WHERE project_id=?1 ORDER BY id DESC LIMIT 100",
    )?;
    let v = s
        .query_map([project_id], |r| {
            Ok(Deployment {
                id: r.get(0)?,
                project_id: r.get(1)?,
                kind: r.get(2)?,
                started_at: r.get(3)?,
                finished_at: r.get(4)?,
                status: r.get(5)?,
                from_commit: r.get(6)?,
                to_commit: r.get(7)?,
                log: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

pub fn get_deployment(db: &Db, id: i64) -> Result<Deployment> {
    let pid: i64 = db
        .lock()
        .query_row("SELECT project_id FROM deployments WHERE id=?1", [id], |r| r.get(0))
        .map_err(|_| AppError::new("not_found", "Riwayat deployment tidak ditemukan."))?;
    history(db, pid)?.into_iter().find(|d| d.id == id).ok_or_else(|| AppError::new("not_found", "Riwayat deployment tidak ditemukan."))
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct GitStatus {
    pub branch: String,
    pub head: String,
    pub head_subject: String,
    pub head_author: String,
    pub head_date: String,
    pub dirty: bool,
    pub changed_files: Vec<String>,
    pub ahead_behind: Option<(i64, i64)>,
}

pub fn git_status(conn: &Conn, p: &Project, fetch: bool) -> Result<GitStatus> {
    let q = shell_quote(&p.path);
    if fetch {
        conn.run(&format!("git -C {q} fetch --prune origin 2>&1"), false, 120)?;
    }
    let script = format!(
        "git -C {q} rev-parse --abbrev-ref HEAD && git -C {q} log -1 --format='%H%x09%an%x09%ad%x09%s' --date=iso && echo '@@status' && git -C {q} status --porcelain=v1 -uno && echo '@@ab' && (git -C {q} rev-list --left-right --count HEAD...origin/{} 2>/dev/null || true)",
        shell_quote(&p.branch).trim_matches('\'')
    );
    let out = conn.run_ok(&script, false, "Membaca status Git")?;
    parse_git(&out)
}

pub fn parse_git(out: &str) -> Result<GitStatus> {
    let mut it = out.lines();
    let branch = it.next().unwrap_or("").trim().to_string();
    let log = it.next().unwrap_or("");
    let mut lp = log.splitn(4, '\t');
    let mut st = GitStatus { branch, head: lp.next().unwrap_or("").into(), head_author: lp.next().unwrap_or("").into(), head_date: lp.next().unwrap_or("").into(), head_subject: lp.next().unwrap_or("").into(), ..Default::default() };
    if st.head.is_empty() {
        return Err(AppError::new("not_a_repo", "Path ini bukan repository Git atau belum ada commit."));
    }
    let mut mode = "";
    for l in it {
        match l.trim() {
            "@@status" => mode = "status",
            "@@ab" => mode = "ab",
            "" => {}
            t if mode == "status" => st.changed_files.push(t.to_string()),
            t if mode == "ab" => {
                let v: Vec<i64> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if v.len() == 2 {
                    st.ahead_behind = Some((v[0], v[1]));
                }
            }
            _ => {}
        }
    }
    st.dirty = !st.changed_files.is_empty();
    Ok(st)
}

struct Runner<'a> {
    app: &'a AppHandle,
    db: &'a Db,
    dep_id: i64,
    log: String,
}

impl Runner<'_> {
    fn line(&mut self, s: &str) {
        let s = crate::security::redact::redact(s);
        self.log.push_str(&s);
        self.log.push('\n');
        let _ = self.app.emit("deploy-log", serde_json::json!({ "id": self.dep_id, "line": s }));
    }
    fn flush(&self, status: &str, to: Option<&str>) {
        let _ = self.db.lock().execute(
            "UPDATE deployments SET log=?1, status=?2, to_commit=COALESCE(?3,to_commit), finished_at=CASE WHEN ?2='running' THEN NULL ELSE strftime('%Y-%m-%dT%H:%M:%SZ','now') END WHERE id=?4",
            params![self.log, status, to, self.dep_id],
        );
    }
    fn step(&mut self, conn: &Conn, title: &str, script: &str, sudo: bool, timeout: u64) -> Result<String> {
        self.line(&format!("$ {title}"));
        let o = conn.run(script, sudo, timeout)?;
        for l in o.stdout.lines().chain(o.stderr.lines()).take(200) {
            self.line(&format!("  {l}"));
        }
        if o.exit != 0 {
            self.line(&format!("✗ gagal (exit {})", o.exit));
            return Err(AppError::new("command_failed", format!("Langkah '{title}' gagal (exit {}).", o.exit)));
        }
        Ok(o.stdout)
    }
}

fn post_steps(r: &mut Runner, conn: &Conn, p: &Project) -> Result<()> {
    let q = shell_quote(&p.path);
    for s in &p.build_steps {
        if risk::classify(s).risk == Risk::Blocked {
            return Err(AppError::denied(format!("Langkah build diblokir: {s}")));
        }
        r.step(conn, s, &format!("cd {q} && {s}"), false, 1800)?;
    }
    if let Some(unit) = &p.restart_service {
        let qu = shell_quote(unit);
        r.step(conn, &format!("systemctl restart {unit}"), &format!("systemctl restart {qu}"), true, 90)?;
        let st = r.step(conn, &format!("systemctl is-active {unit}"), &format!("systemctl is-active {qu}"), false, 20)?;
        if st.trim() != "active" {
            return Err(AppError::new("verify_failed", format!("Service {unit} tidak aktif setelah restart.")));
        }
    }
    if let Some(url) = &p.verify_url {
        r.line(&format!("$ verifikasi {url}"));
        let mut last = None;
        for i in 0..3 {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
            let c = crate::website::check_url(url);
            r.line(&format!("  percobaan {}: status {:?} {}", i + 1, c.status, c.error.clone().unwrap_or_default()));
            if c.ok {
                last = Some(true);
                break;
            }
            last = Some(false);
        }
        if last != Some(true) {
            return Err(AppError::new("verify_failed", "Verifikasi website gagal setelah deployment."));
        }
    }
    Ok(())
}

pub fn deploy(app: &AppHandle, conn: &Conn, db: &Db, p: &Project, confirm_migrations: bool) -> Result<Deployment> {
    let has_migration = p.build_steps.iter().any(|s| risk::is_migration_step(s));
    if has_migration && !confirm_migrations {
        return Err(AppError::denied("Langkah build mengandung migrasi database. Centang konfirmasi migrasi untuk melanjutkan."));
    }
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let dep_id = {
        let c = db.lock();
        c.execute("INSERT INTO deployments (project_id,kind,started_at,status) VALUES (?1,'deploy',?2,'running')", params![p.id, now])?;
        c.last_insert_rowid()
    };
    let mut r = Runner { app, db, dep_id, log: String::new() };
    let q = shell_quote(&p.path);
    let qb = shell_quote(&p.branch);
    let res = (|| -> Result<(String, String)> {
        r.line(&format!("Deploy {} → branch {}", p.name, p.branch));
        let st = r.step(conn, "git status (cek perubahan lokal)", &format!("git -C {q} status --porcelain=v1 -uno"), false, 60)?;
        if !st.trim().is_empty() {
            r.line("✗ Ada perubahan lokal yang belum di-commit. Deploy dibatalkan agar tidak menimpa perubahan.");
            return Err(AppError::new("dirty_tree", "Working tree memiliki perubahan lokal. Deploy dibatalkan."));
        }
        let prev = r.step(conn, "git rev-parse HEAD", &format!("git -C {q} rev-parse HEAD"), false, 30)?.trim().to_string();
        let _ = db.lock().execute("UPDATE deployments SET from_commit=?1 WHERE id=?2", params![prev, dep_id]);
        r.step(conn, "git fetch --prune origin", &format!("git -C {q} fetch --prune origin"), false, 300)?;
        r.step(conn, &format!("git checkout {}", p.branch), &format!("git -C {q} checkout {qb}"), false, 60)?;
        r.step(conn, "git merge --ff-only", &format!("git -C {q} merge --ff-only origin/{}", p.branch), false, 120)?;
        let new = r.step(conn, "git rev-parse HEAD", &format!("git -C {q} rev-parse HEAD"), false, 30)?.trim().to_string();
        post_steps(&mut r, conn, p)?;
        Ok((prev, new))
    })();
    finish(&mut r, db, dep_id, res.map(|(_, n)| n))
}

fn finish(r: &mut Runner, db: &Db, dep_id: i64, res: Result<String>) -> Result<Deployment> {
    match res {
        Ok(new) => {
            r.line("✓ Deployment selesai dan terverifikasi.");
            r.flush("success", Some(&new));
        }
        Err(e) => {
            r.line(&format!("✗ Deployment gagal: {}", e.message));
            r.line("Gunakan Rollback bila aplikasi bermasalah.");
            r.flush("failed", None);
        }
    }
    get_deployment(db, dep_id)
}

/// Rollback ke commit sebelum deployment tertentu (detached HEAD; tidak ada reset --hard).
pub fn rollback(app: &AppHandle, conn: &Conn, db: &Db, p: &Project, target: &Deployment, confirm_migrations: bool) -> Result<Deployment> {
    let Some(commit) = target.from_commit.clone() else {
        return Err(AppError::invalid("Deployment ini tidak memiliki commit sebelumnya untuk rollback."));
    };
    let commit = validate::commit(&commit)?;
    if p.build_steps.iter().any(|s| risk::is_migration_step(s)) && !confirm_migrations {
        return Err(AppError::denied("Langkah build mengandung migrasi database. Konfirmasi diperlukan; rollback kode tidak membatalkan migrasi."));
    }
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let dep_id = {
        let c = db.lock();
        c.execute("INSERT INTO deployments (project_id,kind,started_at,status,from_commit) VALUES (?1,'rollback',?2,'running',?3)", params![p.id, now, target.to_commit])?;
        c.last_insert_rowid()
    };
    let mut r = Runner { app, db, dep_id, log: String::new() };
    let q = shell_quote(&p.path);
    let res = (|| -> Result<String> {
        r.line(&format!("Rollback {} → {}", p.name, &commit[..commit.len().min(10)]));
        let st = r.step(conn, "git status (cek perubahan lokal)", &format!("git -C {q} status --porcelain=v1 -uno"), false, 60)?;
        if !st.trim().is_empty() {
            return Err(AppError::new("dirty_tree", "Working tree memiliki perubahan lokal. Rollback dibatalkan."));
        }
        r.step(conn, "git checkout --detach", &format!("git -C {q} checkout --detach {}", shell_quote(&commit)), false, 60)?;
        post_steps(&mut r, conn, p)?;
        Ok(commit.clone())
    })();
    finish(&mut r, db, dep_id, res)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ProjectInput {
        ProjectInput {
            id: None,
            server_id: 1,
            name: "Web".into(),
            path: "/var/www/app".into(),
            repo_url: None,
            branch: "main".into(),
            build_steps: vec!["npm ci".into(), "npm run build".into()],
            restart_service: Some("pm2-app".into()),
            verify_url: Some("https://example.com".into()),
        }
    }

    #[test]
    fn validates_project() {
        let p = input().validate().unwrap();
        assert_eq!(p.restart_service.as_deref(), Some("pm2-app.service"));
        let mut i = input();
        i.build_steps.push("rm -rf /".into());
        assert!(i.validate().is_err());
        let mut i = input();
        i.path = "/etc".into();
        assert!(i.validate().is_err());
        let mut i = input();
        i.branch = "--evil".into();
        assert!(i.validate().is_err());
        let mut i = input();
        i.path = "/var/www/../etc".into();
        assert!(i.validate().is_err());
    }

    #[test]
    fn project_crud_and_history() {
        let db = Db::memory().unwrap();
        let p = save_project(&db, &input()).unwrap();
        assert_eq!(projects(&db, 1).unwrap().len(), 1);
        assert_eq!(p.build_steps.len(), 2);
        assert!(history(&db, p.id).unwrap().is_empty());
        delete_project(&db, p.id).unwrap();
        assert!(project(&db, p.id).is_err());
    }

    #[test]
    fn parses_git_output() {
        let out = "main\nabc123\tAlice\t2026-01-01 10:00:00 +0000\tFix bug\n@@status\n M app.js\n@@ab\n0\t2\n";
        let s = parse_git(out).unwrap();
        assert_eq!(s.branch, "main");
        assert_eq!(s.head_subject, "Fix bug");
        assert!(s.dirty);
        assert_eq!(s.ahead_behind, Some((0, 2)));
        assert!(parse_git("main\n\n").is_err());
    }
}
