use crate::{
    generation::{self, PreparedBatch},
    library::{Character, CharacterInput, Library, Take},
    project::{self, ProjectStatus, Saver, Session, SharedProject},
    project_open::{self, ProjectOpenRequests},
    types::{EditorState, SynthesisInput, TakeRequest},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::FsExt;

struct Desktop {
    project: SharedProject,
    saver: Saver,
    switching: AtomicBool,
    previews_running: AtomicUsize,
    batch: Mutex<Option<PreparedBatch>>,
    inference: Arc<crate::inference::Inference>,
}
type CommandResult<T> = Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
impl Desktop {
    fn read<T>(&self, read: impl FnOnce(&Session) -> anyhow::Result<T>) -> CommandResult<T> {
        let guard = self.project.lock().map_err(err)?;
        if self.switching.load(Ordering::SeqCst) {
            return Err("プロジェクトを切り替え中です".into());
        }
        read(
            guard
                .as_ref()
                .ok_or("プロジェクトを作成または開いてください")?,
        )
        .map_err(err)
    }
    fn change<T>(
        &self,
        change: impl FnOnce(&Library) -> Result<T, crate::error::AppError>,
    ) -> CommandResult<T> {
        self.change_session(|session| change(&session.library))
    }
    fn change_session<T>(
        &self,
        change: impl FnOnce(&Session) -> Result<T, crate::error::AppError>,
    ) -> CommandResult<T> {
        let mut guard = self.project.lock().map_err(err)?;
        if self.switching.load(Ordering::SeqCst) {
            return Err("プロジェクトを切り替え中です".into());
        }
        let session = guard
            .as_mut()
            .ok_or("プロジェクトを作成または開いてください")?;
        let result = change(session).map_err(err)?;
        session.changed();
        self.saver.changed().map_err(err)?;
        Ok(result)
    }
}
#[tauri::command]
fn is_mobile() -> bool {
    cfg!(mobile)
}
#[tauri::command]
fn inference_status(state: State<Desktop>) -> crate::types::InferenceStatus {
    state.inference.status()
}
#[tauri::command]
fn retry_inference(state: State<Desktop>) -> CommandResult<()> {
    state.inference.retry().map_err(err)
}
#[tauri::command]
fn project_status(state: State<Desktop>) -> CommandResult<ProjectStatus> {
    project::status(&state.project).map_err(err)
}

#[tauri::command]
async fn flush_project(state: State<'_, Desktop>) -> CommandResult<()> {
    let receiver = state.saver.flush_sender().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || receiver.recv().map_err(err).and_then(|r| r))
        .await
        .map_err(err)?
}

#[tauri::command]
async fn choose_project(
    app: tauri::AppHandle,
    state: State<'_, Desktop>,
    create: bool,
) -> CommandResult<bool> {
    switch_project(app, state, create, None).await
}

#[tauri::command]
fn take_project_open_requests(state: State<ProjectOpenRequests>) -> Vec<std::path::PathBuf> {
    state.take()
}

#[tauri::command]
async fn open_project(
    app: tauri::AppHandle,
    state: State<'_, Desktop>,
    path: std::path::PathBuf,
) -> CommandResult<bool> {
    if !path.is_absolute() || !project_open::is_project(&path) {
        return Err(".iro プロジェクトのパスが不正です".into());
    }
    switch_project(app, state, false, Some(path)).await
}

async fn switch_project(
    app: tauri::AppHandle,
    state: State<'_, Desktop>,
    create: bool,
    path: Option<std::path::PathBuf>,
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
    let path = if let Some(path) = path {
        path
    } else {
        let dialog_app = app.clone();
        let path = tauri::async_runtime::spawn_blocking(move || {
            let dialog = dialog_app.dialog().file();
            if create {
                dialog
                    .add_filter("irodoru project", &["iro"])
                    .set_file_name("project.iro")
                    .blocking_save_file()
            } else {
                dialog
                    .add_filter("irodoru project", &["iro", "zip"])
                    .blocking_pick_file()
            }
        })
        .await
        .map_err(err)?;
        let Some(path) = path else {
            return Ok(false);
        };
        let path = path.into_path().map_err(|_| {
            "この保存先はローカルファイルとして開けません。端末上のフォルダを選んでください"
                .to_string()
        })?;
        path
    };
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
    if create {
        state.saver.changed().map_err(err)?;
    }
    app.emit(
        "project-status",
        project::status(&state.project).map_err(err)?,
    )
    .map_err(err)?;
    Ok(true)
}

#[tauri::command]
fn fetch_characters(state: State<Desktop>) -> CommandResult<Vec<Character>> {
    state.read(|s| Ok(s.library.characters()?))
}
#[tauri::command]
fn save_character(
    state: State<Desktop>,
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
fn remove_character(state: State<Desktop>, id: String) -> CommandResult<()> {
    state.change(|library| library.delete_character(&id))
}
#[tauri::command]
fn fetch_voices(state: State<Desktop>) -> CommandResult<Vec<String>> {
    state.read(|s| Ok(s.library.voices()?))
}
#[tauri::command]
fn upload_voice(state: State<Desktop>, name: String, wav: Vec<u8>) -> CommandResult<()> {
    state.change(|library| library.save_voice(&name, &wav))
}
#[tauri::command]
fn fetch_takes(state: State<Desktop>) -> CommandResult<Vec<Take>> {
    state.read(|s| Ok(s.library.takes()?))
}
#[tauri::command]
fn remove_take(state: State<Desktop>, id: String) -> CommandResult<()> {
    state.change(|library| library.delete_take(&id))
}
#[tauri::command]
fn take_audio_path(state: State<Desktop>, id: String) -> CommandResult<String> {
    state.read(|s| Ok(s.library.audio_path(&id)?.to_string_lossy().into_owned()))
}
#[tauri::command]
fn load_editor(state: State<Desktop>) -> CommandResult<EditorState> {
    state.read(|s| Ok(s.library.editor()?))
}
#[tauri::command]
fn save_editor(state: State<Desktop>, editor: EditorState) -> CommandResult<()> {
    state.change(|library| library.save_editor(&editor))
}
#[tauri::command]
fn prepare_generation(state: State<Desktop>, requests: Vec<TakeRequest>) -> CommandResult<String> {
    let mut batch = state.batch.lock().map_err(err)?;
    state.read(|s| PreparedBatch::prepare(&mut batch, &s.library, requests))
}
#[tauri::command]
async fn generate_take(
    state: State<'_, Desktop>,
    batch: String,
    index: usize,
) -> CommandResult<Take> {
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
fn finish_generation(state: State<Desktop>, batch: String) -> CommandResult<()> {
    PreparedBatch::finish(&mut *state.batch.lock().map_err(err)?, &batch);
    Ok(())
}
#[tauri::command]
async fn export_take(
    app: tauri::AppHandle,
    state: State<'_, Desktop>,
    id: String,
) -> CommandResult<()> {
    let (wav, filename) =
        state.read(|s| Ok((s.library.audio(&id)?, s.library.export_filename(&id)?)))?;
    save_wav(app, wav, filename).await
}
async fn save_wav(app: tauri::AppHandle, wav: Vec<u8>, filename: String) -> CommandResult<()> {
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .set_file_name(filename)
            .add_filter("WAV", &["wav"])
            .blocking_save_file()
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
    state: State<'_, Desktop>,
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
fn preview_audio_path(state: State<Desktop>, id: String) -> CommandResult<String> {
    state.read(|s| {
        let path = match s.library.character_preview_path(&id)? {
            Some(path) => path,
            None => s.previews.audio_path(&id)?,
        };
        Ok(path.to_string_lossy().into_owned())
    })
}
#[tauri::command]
fn character_preview(state: State<Desktop>, id: String) -> CommandResult<Option<String>> {
    state.read(|s| Ok(s.library.character_preview_path(&id)?.map(|_| id)))
}
#[tauri::command]
fn remove_preview(state: State<Desktop>, id: String) -> CommandResult<()> {
    state.read(|s| Ok(s.previews.delete_take(&id)?))
}
#[tauri::command]
async fn export_preview(
    app: tauri::AppHandle,
    state: State<'_, Desktop>,
    id: String,
) -> CommandResult<()> {
    let (wav, filename) = state.read(|s| {
        if let Some(path) = s.library.character_preview_path(&id)? {
            return Ok((std::fs::read(path)?, format!("sample-{id}.wav")));
        }
        Ok((s.previews.audio(&id)?, s.previews.export_filename(&id)?))
    })?;
    save_wav(app, wav, filename).await
}
#[cfg(desktop)]
fn queue_project_opens(
    app: &tauri::AppHandle,
    paths: impl IntoIterator<Item = std::path::PathBuf>,
) {
    app.state::<ProjectOpenRequests>().push(paths);
    let _ = app.emit("project-open-requested", ());
}

pub fn run() {
    let builder = tauri::Builder::default().manage(ProjectOpenRequests::default());
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
        queue_project_opens(
            app,
            project_open::argument_paths(
                args.into_iter().map(std::ffi::OsString::from),
                std::path::Path::new(&cwd),
            ),
        );
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.state::<ProjectOpenRequests>()
                .push(project_open::argument_paths(
                    std::env::args_os(),
                    &std::env::current_dir()?,
                ));
            let project: SharedProject = Arc::new(Mutex::new(None));
            let save_handle = app.handle().clone();
            let saver = Saver::start(project.clone(), move |status| {
                let _ = save_handle.emit("project-status", status);
            })?;
            let handle = app.handle().clone();
            app.manage(Desktop {
                project,
                saver,
                switching: AtomicBool::new(false),
                previews_running: AtomicUsize::new(0),
                batch: Mutex::new(None),
                inference: Arc::new(crate::inference::Inference::start_with_progress(
                    move |progress| {
                        Ok(irodoru_engine::EngineConfig::anime(
                            crate::resources::models(&handle, progress)?,
                        ))
                    },
                )?),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            is_mobile,
            inference_status,
            retry_inference,
            project_status,
            choose_project,
            open_project,
            take_project_open_requests,
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
        .build(tauri::generate_context!())
        .expect("desktop application failed")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                queue_project_opens(
                    _app,
                    urls.into_iter().filter_map(|url| url.to_file_path().ok()),
                );
            }
        });
}
