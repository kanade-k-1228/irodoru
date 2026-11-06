use crate::{
    generation::{self, PreparedBatch},
    library::{Character, CharacterInput, Library, Take},
    types::{EditorState, SynthesisInput, TakeRequest},
};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::FsExt;

struct Desktop {
    library: Mutex<Library>,
    previews: Mutex<Library>,
    batch: Mutex<Option<PreparedBatch>>,
    inference: Arc<crate::inference::Inference>,
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
fn inference_status(state: State<Desktop>) -> crate::types::InferenceStatus {
    state.inference.status()
}
#[tauri::command]
fn fetch_characters(state: State<Desktop>) -> CommandResult<Vec<Character>> {
    state.library.lock().map_err(err)?.characters().map_err(err)
}
#[tauri::command]
fn save_character(
    state: State<Desktop>,
    input: CharacterInput,
    id: Option<String>,
) -> CommandResult<Character> {
    let library = state.library.lock().map_err(err)?;
    match id {
        Some(id) => library.update_character(id, input),
        None => library.create_character(input),
    }
    .map_err(err)
}
#[tauri::command]
fn remove_character(state: State<Desktop>, id: String) -> CommandResult<()> {
    state
        .library
        .lock()
        .map_err(err)?
        .delete_character(&id)
        .map_err(err)
}
#[tauri::command]
fn fetch_voices(state: State<Desktop>) -> CommandResult<Vec<String>> {
    state.library.lock().map_err(err)?.voices().map_err(err)
}
#[tauri::command]
fn upload_voice(state: State<Desktop>, name: String, wav: Vec<u8>) -> CommandResult<()> {
    state
        .library
        .lock()
        .map_err(err)?
        .save_voice(&name, &wav)
        .map_err(err)
}
#[tauri::command]
fn fetch_takes(state: State<Desktop>) -> CommandResult<Vec<Take>> {
    state.library.lock().map_err(err)?.takes().map_err(err)
}
#[tauri::command]
fn remove_take(state: State<Desktop>, id: String) -> CommandResult<()> {
    state
        .library
        .lock()
        .map_err(err)?
        .delete_take(&id)
        .map_err(err)
}
#[tauri::command]
fn take_audio_path(state: State<Desktop>, id: String) -> CommandResult<String> {
    Ok(state
        .library
        .lock()
        .map_err(err)?
        .audio_path(&id)
        .map_err(err)?
        .to_string_lossy()
        .into_owned())
}
#[tauri::command]
fn load_editor(state: State<Desktop>) -> CommandResult<EditorState> {
    state.library.lock().map_err(err)?.editor().map_err(err)
}
#[tauri::command]
fn save_editor(state: State<Desktop>, editor: EditorState) -> CommandResult<()> {
    state
        .library
        .lock()
        .map_err(err)?
        .save_editor(&editor)
        .map_err(err)
}
#[tauri::command]
fn prepare_generation(state: State<Desktop>, requests: Vec<TakeRequest>) -> CommandResult<String> {
    let mut batch = state.batch.lock().map_err(err)?;
    PreparedBatch::prepare(&mut batch, &*state.library.lock().map_err(err)?, requests).map_err(err)
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
    state
        .library
        .lock()
        .map_err(err)?
        .save_take(&take, &wav)
        .map_err(err)?;
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
    let (wav, filename) = {
        let library = state.library.lock().map_err(err)?;
        (
            library.audio(&id).map_err(err)?,
            library.export_filename(&id).map_err(err)?,
        )
    };
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
    let prepared = generation::prepare_preview(
        &*state.library.lock().map_err(err)?,
        request,
        description,
        voice,
    )
    .map_err(err)?;
    let inference = state.inference.clone();
    let (take, wav) = tauri::async_runtime::spawn_blocking(move || {
        generation::synthesize(prepared, |job| inference.synthesize(job))
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    state
        .previews
        .lock()
        .map_err(err)?
        .save_take(&take, &wav)
        .map_err(err)?;
    Ok(take.id)
}
#[tauri::command]
fn preview_audio_path(state: State<Desktop>, id: String) -> CommandResult<String> {
    Ok(state
        .previews
        .lock()
        .map_err(err)?
        .audio_path(&id)
        .map_err(err)?
        .to_string_lossy()
        .into_owned())
}
#[tauri::command]
fn remove_preview(state: State<Desktop>, id: String) -> CommandResult<()> {
    state
        .previews
        .lock()
        .map_err(err)?
        .delete_take(&id)
        .map_err(err)
}
#[tauri::command]
async fn export_preview(
    app: tauri::AppHandle,
    state: State<'_, Desktop>,
    id: String,
) -> CommandResult<()> {
    let (wav, filename) = {
        let library = state.previews.lock().map_err(err)?;
        (
            library.audio(&id).map_err(err)?,
            library.export_filename(&id).map_err(err)?,
        )
    };
    save_wav(app, wav, filename).await
}
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let library = Library::open(&app.path().app_local_data_dir()?)?;
            let preview_root = app.path().app_local_data_dir()?.join("previews");
            if preview_root.exists() {
                std::fs::remove_dir_all(&preview_root)?;
            }
            let previews = Library::open(&preview_root)?;
            let handle = app.handle().clone();
            app.manage(Desktop {
                previews: Mutex::new(previews),
                library: Mutex::new(library),
                batch: Mutex::new(None),
                inference: Arc::new(crate::inference::Inference::start_with(move || {
                    Ok(irodoru_engine::EngineConfig::anime(
                        crate::resources::models(&handle)?,
                    ))
                })?),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            is_mobile,
            inference_status,
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
            remove_preview,
            export_preview
        ])
        .run(tauri::generate_context!())
        .expect("desktop application failed");
}
