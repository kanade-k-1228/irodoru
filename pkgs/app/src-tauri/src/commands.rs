use crate::{
    generation::{self, PreparedBatch},
    library::{Character, CharacterInput, Take},
    project::{self, ProjectStatus, Session},
    types::{EditorState, SynthesisInput, TakeRequest},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::FsExt;

struct Desktop {
    workspaces: Arc<crate::workspaces::Workspaces>,
    inference: Arc<crate::inference::Inference>,
}

// Resolve project state from the invoking window, never from a client-supplied ID.
#[derive(Clone)]
struct WindowState {
    workspace: Arc<crate::workspaces::Workspace>,
    workspaces: Arc<crate::workspaces::Workspaces>,
    inference: Arc<crate::inference::Inference>,
    label: String,
}
impl std::ops::Deref for WindowState {
    type Target = crate::workspaces::Workspace;
    fn deref(&self) -> &Self::Target {
        &self.workspace
    }
}
impl<'de, R: tauri::Runtime> tauri::ipc::CommandArg<'de, R> for WindowState {
    fn from_command(
        command: tauri::ipc::CommandItem<'de, R>,
    ) -> Result<Self, tauri::ipc::InvokeError> {
        let webview = command.message.webview_ref();
        let label = webview.window().label().to_owned();
        let app = webview.state::<Desktop>();
        Ok(Self {
            workspace: app.workspaces.get(&label).map_err(err)?,
            workspaces: app.workspaces.clone(),
            inference: app.inference.clone(),
            label,
        })
    }
}
type CommandResult<T> = Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[tauri::command]
fn is_mobile() -> bool {
    cfg!(mobile)
}
#[tauri::command]
fn inference_status(state: WindowState) -> crate::types::InferenceStatus {
    state.inference.status()
}
#[tauri::command]
fn project_status(state: WindowState) -> CommandResult<ProjectStatus> {
    project::status(&state.project).map_err(err)
}

#[tauri::command]
async fn flush_project(state: WindowState) -> CommandResult<()> {
    let receiver = state.saver.flush_sender().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || receiver.recv().map_err(err).and_then(|r| r))
        .await
        .map_err(err)?
}

#[tauri::command]
async fn choose_project(
    app: tauri::AppHandle,
    _window: tauri::WebviewWindow,
    state: WindowState,
    create: bool,
) -> CommandResult<bool> {
    if state.switching.swap(true, Ordering::SeqCst) {
        return Err("プロジェクトを切り替え中です".into());
    }
    struct Reset<'a>(&'a AtomicBool);
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _reset = Reset(&state.switching);
    if state.batch.lock().map_err(err)?.is_some()
        || state.previews_running.load(Ordering::SeqCst) != 0
    {
        return Err("生成完了後にプロジェクトを切り替えてください".into());
    }
    flush_project(state.clone()).await?;
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        let dialog = dialog_app
            .dialog()
            .file()
            .add_filter("irodoru project", &["zip"]);
        #[cfg(desktop)]
        let dialog = dialog.set_parent(&_window);
        if create {
            dialog.set_file_name("project.zip").blocking_save_file()
        } else {
            dialog.blocking_pick_file()
        }
    })
    .await
    .map_err(err)?;
    let Some(path) = path else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|_| {
        "この保存先はローカルファイルとして開けません。端末上のフォルダを選んでください".to_string()
    })?;
    let reservation = state.workspaces.reserve(&state.label, &path).map_err(err)?;
    let path = reservation.path.clone();
    let session = tauri::async_runtime::spawn_blocking(move || {
        if create {
            Session::create(path)
        } else {
            Session::open(path)
        }
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    app.asset_protocol_scope()
        .allow_directory(session.library.root(), true)
        .map_err(err)?;
    app.asset_protocol_scope()
        .allow_directory(session.previews.root(), true)
        .map_err(err)?;
    {
        let mut current = state.project.lock().map_err(err)?;
        *current = Some(session);
    }
    reservation.commit();
    if create {
        state.saver.changed().map_err(err)?;
    }
    app.emit_to(
        state.label.clone(),
        "project-status",
        project::status(&state.project).map_err(err)?,
    )
    .map_err(err)?;
    Ok(true)
}

#[tauri::command]
fn fetch_characters(state: WindowState) -> CommandResult<Vec<Character>> {
    state.read(|s| Ok(s.library.characters()?))
}
#[tauri::command]
fn save_character(
    state: WindowState,
    input: CharacterInput,
    id: Option<String>,
    preview: Option<String>,
) -> CommandResult<Character> {
    state.change_session(|session| {
        let character = match id {
            Some(id) => session.library.update_character(id, input)?,
            None => session.library.create_character(input)?,
        };
        if let Some(preview) = preview {
            session
                .library
                .save_character_preview(&character.id, &session.previews, &preview)?;
        }
        Ok(character)
    })
}
#[tauri::command]
fn remove_character(state: WindowState, id: String) -> CommandResult<()> {
    state.change(|library| library.delete_character(&id))
}
#[tauri::command]
fn fetch_voices(state: WindowState) -> CommandResult<Vec<String>> {
    state.read(|s| Ok(s.library.voices()?))
}
#[tauri::command]
fn upload_voice(state: WindowState, name: String, wav: Vec<u8>) -> CommandResult<()> {
    state.change(|library| library.save_voice(&name, &wav))
}
#[tauri::command]
fn fetch_takes(state: WindowState) -> CommandResult<Vec<Take>> {
    state.read(|s| Ok(s.library.takes()?))
}
#[tauri::command]
fn remove_take(state: WindowState, id: String) -> CommandResult<()> {
    state.change(|library| library.delete_take(&id))
}
#[tauri::command]
fn take_audio_path(state: WindowState, id: String) -> CommandResult<String> {
    state.read(|s| Ok(s.library.audio_path(&id)?.to_string_lossy().into_owned()))
}
#[tauri::command]
fn load_editor(state: WindowState) -> CommandResult<EditorState> {
    state.read(|s| Ok(s.library.editor()?))
}
#[tauri::command]
fn save_editor(state: WindowState, editor: EditorState) -> CommandResult<()> {
    state.change(|library| library.save_editor(&editor))
}
#[tauri::command]
fn prepare_generation(state: WindowState, requests: Vec<TakeRequest>) -> CommandResult<String> {
    let mut batch = state.batch.lock().map_err(err)?;
    state.read(|s| PreparedBatch::prepare(&mut batch, &s.library, requests))
}
#[tauri::command]
async fn generate_take(state: WindowState, batch: String, index: usize) -> CommandResult<Take> {
    let prepared = state
        .batch
        .lock()
        .map_err(err)?
        .as_mut()
        .ok_or("生成リクエストが見つかりません")?
        .take(&batch, index)
        .map_err(err)?;
    let inference = state.inference.clone();
    let (take, wav) = tauri::async_runtime::spawn_blocking(move || {
        generation::synthesize(prepared, |job| inference.synthesize(job))
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    state.change(|library| library.save_take(&take, &wav))?;
    Ok(take)
}
#[tauri::command]
fn finish_generation(state: WindowState, batch: String) -> CommandResult<()> {
    PreparedBatch::finish(&mut *state.batch.lock().map_err(err)?, &batch);
    Ok(())
}
#[tauri::command]
async fn export_take(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: WindowState,
    id: String,
) -> CommandResult<()> {
    let (wav, filename) =
        state.read(|s| Ok((s.library.audio(&id)?, s.library.export_filename(&id)?)))?;
    save_wav(app, window, wav, filename).await
}
async fn save_wav(
    app: tauri::AppHandle,
    _window: tauri::WebviewWindow,
    wav: Vec<u8>,
    filename: String,
) -> CommandResult<()> {
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        let dialog = dialog_app
            .dialog()
            .file()
            .set_file_name(filename)
            .add_filter("WAV", &["wav"]);
        #[cfg(desktop)]
        let dialog = dialog.set_parent(&_window);
        dialog.blocking_save_file()
    })
    .await
    .map_err(err)?;
    if let Some(path) = path {
        tauri::async_runtime::spawn_blocking(move || -> CommandResult<()> {
            use std::io::Write;
            let mut options = tauri_plugin_fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            let result = app
                .fs()
                .open(path.clone(), options)
                .and_then(|mut file| file.write_all(&wav));
            #[cfg(target_os = "ios")]
            let _ = app.fs().stop_accessing_security_scoped_resource(path);
            result.map_err(err)
        })
        .await
        .map_err(err)??;
    }
    Ok(())
}
#[tauri::command]
async fn preview_character(
    state: WindowState,
    request: SynthesisInput,
    description: String,
    voice: Option<String>,
) -> CommandResult<String> {
    state.previews_running.fetch_add(1, Ordering::SeqCst);
    struct Reset<'a>(&'a AtomicUsize);
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let _reset = Reset(&state.previews_running);
    let prepared =
        state.read(|s| generation::prepare_preview(&s.library, request, description, voice))?;
    let inference = state.inference.clone();
    let (take, wav) = tauri::async_runtime::spawn_blocking(move || {
        generation::synthesize(prepared, |job| inference.synthesize(job))
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    state.read(|s| Ok(s.previews.save_take(&take, &wav)?))?;
    Ok(take.id)
}
#[tauri::command]
fn preview_audio_path(state: WindowState, id: String) -> CommandResult<String> {
    state.read(|s| {
        let path = match s.library.character_preview_path(&id)? {
            Some(path) => path,
            None => s.previews.audio_path(&id)?,
        };
        Ok(path.to_string_lossy().into_owned())
    })
}
#[tauri::command]
fn character_preview(state: WindowState, id: String) -> CommandResult<Option<String>> {
    state.read(|s| Ok(s.library.character_preview_path(&id)?.map(|_| id)))
}
#[tauri::command]
fn remove_preview(state: WindowState, id: String) -> CommandResult<()> {
    state.read(|s| Ok(s.previews.delete_take(&id)?))
}
#[tauri::command]
async fn export_preview(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: WindowState,
    id: String,
) -> CommandResult<()> {
    let (wav, filename) = state.read(|s| {
        if let Some(path) = s.library.character_preview_path(&id)? {
            return Ok((std::fs::read(path)?, format!("sample-{id}.wav")));
        }
        Ok((s.previews.audio(&id)?, s.previews.export_filename(&id)?))
    })?;
    save_wav(app, window, wav, filename).await
}
#[tauri::command]
async fn new_window(app: tauri::AppHandle) -> CommandResult<()> {
    #[cfg(mobile)]
    {
        let _ = app;
        Err("この端末では複数ウィンドウを利用できません".into())
    }
    #[cfg(desktop)]
    {
        let mut config = app
            .config()
            .app
            .windows
            .first()
            .ok_or("ウィンドウ設定がありません")?
            .clone();
        let label = format!("project-{}", crate::library::new_id());
        config.label = label.clone();
        let save_handle = app.clone();
        let save_label = label.clone();
        let state = app.state::<Desktop>();
        state
            .workspaces
            .insert(&label, move |status| {
                let _ = save_handle.emit_to(save_label.clone(), "project-status", status);
            })
            .map_err(err)?;
        let result = tauri::WebviewWindowBuilder::from_config(&app, &config)
            .and_then(|builder| builder.build());
        if let Err(error) = result {
            state.workspaces.remove(&label);
            return Err(err(error));
        }
        Ok(())
    }
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.webview_windows().values().next() {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let workspaces = Arc::new(crate::workspaces::Workspaces::default());
            let save_handle = app.handle().clone();
            workspaces.insert("main", move |status| {
                let _ = save_handle.emit_to("main", "project-status", status);
            })?;
            let handle = app.handle().clone();
            app.manage(Desktop {
                workspaces,
                inference: Arc::new(crate::inference::Inference::start_with(move || {
                    Ok(irodoru_engine::EngineConfig::anime(
                        crate::resources::models(&handle)?,
                    ))
                })?),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let workspaces = window.state::<Desktop>().workspaces.clone();
                let label = window.label().to_owned();
                tauri::async_runtime::spawn_blocking(move || workspaces.remove(&label));
            }
        })
        .invoke_handler(tauri::generate_handler![
            is_mobile,
            new_window,
            inference_status,
            project_status,
            choose_project,
            flush_project,
            fetch_characters,
            save_character,
            remove_character,
            fetch_voices,
            upload_voice,
            fetch_takes,
            remove_take,
            take_audio_path,
            load_editor,
            save_editor,
            prepare_generation,
            generate_take,
            finish_generation,
            export_take,
            preview_character,
            preview_audio_path,
            character_preview,
            remove_preview,
            export_preview
        ])
        .run(tauri::generate_context!())
        .expect("desktop application failed");
}
