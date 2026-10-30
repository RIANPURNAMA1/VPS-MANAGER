use super::audit;
use crate::deployment::{self, Deployment, GitStatus, Project, ProjectInput};
use crate::error::Result;
use crate::ssh::blocking;
use crate::state::Shared;
use std::time::Instant;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn deploy_projects(state: State<'_, Shared>, server_id: i64) -> Result<Vec<Project>> {
    let st = state.inner().clone();
    blocking(move || deployment::projects(&st.db, server_id)).await
}

#[tauri::command]
pub async fn deploy_project_save(state: State<'_, Shared>, input: ProjectInput) -> Result<Project> {
    let st = state.inner().clone();
    blocking(move || deployment::save_project(&st.db, &input)).await
}

#[tauri::command]
pub async fn deploy_project_delete(state: State<'_, Shared>, id: i64) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || deployment::delete_project(&st.db, id)).await
}

#[tauri::command]
pub async fn deploy_git_status(state: State<'_, Shared>, project_id: i64, fetch: bool) -> Result<GitStatus> {
    let st = state.inner().clone();
    blocking(move || {
        let p = deployment::project(&st.db, project_id)?;
        deployment::git_status(&*st.conn(p.server_id)?, &p, fetch)
    })
    .await
}

#[tauri::command]
pub async fn deploy_history(state: State<'_, Shared>, project_id: i64) -> Result<Vec<Deployment>> {
    let st = state.inner().clone();
    blocking(move || deployment::history(&st.db, project_id)).await
}

#[tauri::command]
pub async fn deploy_run(app: AppHandle, state: State<'_, Shared>, project_id: i64, confirm_migrations: bool) -> Result<Deployment> {
    let st = state.inner().clone();
    blocking(move || {
        let p = deployment::project(&st.db, project_id)?;
        let c = st.conn(p.server_id)?;
        let t = Instant::now();
        let r = deployment::deploy(&app, &c, &st.db, &p, confirm_migrations);
        let mapped = r.as_ref().map(|d| d.status.clone()).map_err(|e| e.clone());
        match &r {
            Ok(d) if d.status != "success" => crate::storage::logs::record(&st.db, Some((c.profile.id, &c.profile.name)), "deploy.run", "error", Some(1), t.elapsed().as_millis() as i64, &format!("{}: deployment gagal", p.name)),
            _ => audit(&st, &c, "deploy.run", t, &mapped, &format!("{}: deployment sukses", p.name)),
        }
        r
    })
    .await
}

#[tauri::command]
pub async fn deploy_rollback(app: AppHandle, state: State<'_, Shared>, deployment_id: i64, confirm_migrations: bool) -> Result<Deployment> {
    let st = state.inner().clone();
    blocking(move || {
        let target = deployment::get_deployment(&st.db, deployment_id)?;
        let p = deployment::project(&st.db, target.project_id)?;
        let c = st.conn(p.server_id)?;
        let t = Instant::now();
        let r = deployment::rollback(&app, &c, &st.db, &p, &target, confirm_migrations);
        let mapped = r.as_ref().map(|d| d.status.clone()).map_err(|e| e.clone());
        match &r {
            Ok(d) if d.status != "success" => crate::storage::logs::record(&st.db, Some((c.profile.id, &c.profile.name)), "deploy.rollback", "error", Some(1), t.elapsed().as_millis() as i64, &format!("{}: rollback gagal", p.name)),
            _ => audit(&st, &c, "deploy.rollback", t, &mapped, &format!("{}: rollback sukses", p.name)),
        }
        r
    })
    .await
}
