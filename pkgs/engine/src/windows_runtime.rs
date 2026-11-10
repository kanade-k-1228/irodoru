//! Shared Windows ML runtime and its certified execution-provider catalog.
//! ABI: Microsoft.Windows.AI.MachineLearning 2.4.89, WinMLEpCatalog.h (ORT API 27).
use anyhow::{Context, Result, ensure};
use libloading::{Library, Symbol};
use ort::environment::{Environment, EnvironmentBuilder};
use std::{
    ffi::{CStr, c_char, c_void},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use windows::{
    Win32::{
        Security::PSID,
        Storage::Packaging::Appx::*,
        System::{
            Memory::{GetProcessHeap, HEAP_FLAGS, HeapFree},
            WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
        },
    },
    core::{PCWSTR, PWSTR, w},
};

struct SharedRuntime {
    directory: PathBuf,
    // DLLs and the package dependency must outlive every ORT session/provider.
    catalog_library: Library,
    _ort_library: Library,
}
static RUNTIME: OnceLock<std::result::Result<SharedRuntime, String>> = OnceLock::new();
static REGISTERED: Mutex<bool> = Mutex::new(false);

struct Apartment;
impl Apartment {
    fn enter() -> Result<Self> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .context("Initializing Windows ML apartment")?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

fn framework_directory() -> Result<PathBuf> {
    // OS dynamic dependencies are available on our Windows 11 24H2 baseline.
    // Use the publisher-qualified family, never search PATH for runtime DLLs.
    let id = unsafe {
        TryCreatePackageDependency(
            PSID::default(),
            w!("Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe"),
            PACKAGE_VERSION {
                Anonymous: PACKAGE_VERSION_0 {
                    Version: 0x0002_0006_0005_0000,
                },
            },
            PackageDependencyProcessorArchitectures_X64,
            PackageDependencyLifetimeKind_Process,
            PCWSTR::null(),
            CreatePackageDependencyOptions_None,
        )
    }
    .context(
        "Install Windows App SDK Runtime 2.6.5 or later (x64, major version 2) to use Windows ML",
    )?;
    let mut context = PACKAGEDEPENDENCY_CONTEXT::default();
    let mut full_name = PWSTR::null();
    let added = unsafe {
        AddPackageDependency(
            PCWSTR(id.0),
            0,
            AddPackageDependencyOptions_None,
            &mut context,
            Some(&mut full_name),
        )
    };
    unsafe {
        let _ = HeapFree(GetProcessHeap()?, HEAP_FLAGS(0), Some(id.0.cast()));
    }
    added.context("Adding Windows ML framework to the process dependency graph")?;
    // Deliberately retain the process-lifetime dependency: ORT and EPs can keep
    // code/data from this package alive until process exit.
    let result = (|| -> Result<PathBuf> {
        let mut length = 0;
        let first = unsafe { GetPackagePathByFullName(PCWSTR(full_name.0), &mut length, None) };
        ensure!(
            first.0 == 122 && length > 0,
            "Cannot locate Windows ML framework: {}",
            first.0
        );
        let mut path = vec![0u16; length as usize];
        unsafe {
            GetPackagePathByFullName(
                PCWSTR(full_name.0),
                &mut length,
                Some(PWSTR(path.as_mut_ptr())),
            )
        }
        .ok()?;
        use std::os::windows::ffi::OsStringExt;
        let end = path.iter().position(|v| *v == 0).unwrap_or(path.len());
        Ok(PathBuf::from(std::ffi::OsString::from_wide(&path[..end])))
    })();
    unsafe {
        let _ = HeapFree(GetProcessHeap()?, HEAP_FLAGS(0), Some(full_name.0.cast()));
    }
    result
}

fn runtime() -> Result<&'static SharedRuntime> {
    let result = RUNTIME.get_or_init(|| {
        (|| -> Result<SharedRuntime> {
            let directory = framework_directory()?;
            // DLL_LOAD_DIR | DEFAULT_DIRS: resolve dependencies from this signed
            // package and the dependency graph, not the working directory.
            let ort_library = unsafe {
                libloading::os::windows::Library::load_with_flags(
                    directory.join("onnxruntime.dll"),
                    0x1100,
                )
            }
            .context("Loading ONNX Runtime from the Windows ML framework")?;
            let library = unsafe {
                libloading::os::windows::Library::load_with_flags(
                    directory.join("Microsoft.Windows.AI.MachineLearning.dll"),
                    0x1100,
                )
            }
            .context("Loading the Windows ML catalog from its framework package")?;
            Ok(SharedRuntime {
                directory,
                catalog_library: library.into(),
                _ort_library: ort_library.into(),
            })
        })()
        .map_err(|e| format!("{e:#}"))
    });
    result.as_ref().map_err(|e| anyhow::anyhow!("{e}"))
}

// Select the API before creating any ORT object. `true` means the shared
// Windows ML runtime; otherwise `ort` keeps its statically linked CPU runtime.
// Handles are never passed between the two.
static SELECTED: OnceLock<std::result::Result<bool, String>> = OnceLock::new();

fn shared_api() -> Result<ort::sys::OrtApi> {
    let runtime = runtime()?;
    unsafe {
        let get: Symbol<unsafe extern "system" fn() -> *const ort::sys::OrtApiBase> =
            runtime._ort_library.get(b"OrtGetApiBase\0")?;
        let base = get();
        ensure!(!base.is_null(), "Missing ONNX Runtime API base");
        let api = ((*base).GetApi)(ort::sys::ORT_API_VERSION);
        ensure!(!api.is_null(), "ONNX Runtime does not support API 27");
        Ok(std::ptr::read(api))
    }
}

fn select_api(force_cpu: bool) -> Result<bool> {
    let selected = SELECTED.get_or_init(|| {
        if force_cpu {
            return Ok(false);
        }
        match shared_api() {
            Ok(api) => {
                if ort::set_api(api) {
                    Ok(true)
                } else {
                    Err("ONNX Runtime was initialized before runtime selection".into())
                }
            }
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "Windows ML unavailable; using embedded ONNX Runtime CPU");
                Ok(false)
            }
        }
    });
    selected
        .as_ref()
        .copied()
        .map_err(|e| anyhow::anyhow!("{e}"))
}

pub(crate) fn builder() -> Result<EnvironmentBuilder> {
    select_api(false)?;
    Ok(ort::init())
}

pub(crate) fn name() -> &'static str {
    if matches!(SELECTED.get(), Some(Ok(true))) {
        "Windows ML"
    } else {
        "ONNX Runtime（内蔵）"
    }
}

#[repr(C)]
struct EpInfo {
    name: *const c_char,
    version: *const c_char,
    package_family_name: *const c_char,
    library_path: *const c_char,
    package_root_path: *const c_char,
    ready_state: i32,
    certification: i32,
}
type Handle = *mut c_void;
type EnumCallback = unsafe extern "system" fn(Handle, *const EpInfo, *mut c_void) -> i32;
struct Provider {
    handle: Handle,
    name: String,
}
unsafe extern "system" fn collect(ep: Handle, info: *const EpInfo, context: *mut c_void) -> i32 {
    if info.is_null() || ep.is_null() {
        return 1;
    }
    let info = unsafe { &*info };
    if info.certification != 1 || info.name.is_null() {
        return 1;
    }
    // Copy names; provider handles remain valid until the catalog is released.
    let providers = unsafe { &mut *context.cast::<Vec<Provider>>() };
    if let Ok(name) = unsafe { CStr::from_ptr(info.name) }.to_str() {
        providers.push(Provider {
            handle: ep,
            name: name.to_owned(),
        });
    }
    1
}
fn check(hr: i32, operation: &str) -> Result<()> {
    ensure!(hr >= 0, "{operation} failed (HRESULT 0x{:08X})", hr as u32);
    Ok(())
}

pub(crate) fn register_providers(env: &Environment) -> Result<()> {
    if !select_api(false)? {
        return Ok(());
    }
    let mut registered = REGISTERED
        .lock()
        .map_err(|_| anyhow::anyhow!("Windows ML catalog lock poisoned"))?;
    if *registered {
        return Ok(());
    }
    let _apartment = Apartment::enter()?;
    let library = &runtime()?.catalog_library;
    unsafe {
        let create: Symbol<unsafe extern "system" fn(*mut Handle) -> i32> =
            library.get(b"WinMLEpCatalogCreate\0")?;
        let release: Symbol<unsafe extern "system" fn(Handle)> =
            library.get(b"WinMLEpCatalogRelease\0")?;
        let enumerate: Symbol<unsafe extern "system" fn(Handle, EnumCallback, *mut c_void) -> i32> =
            library.get(b"WinMLEpCatalogEnumProviders\0")?;
        let ready: Symbol<unsafe extern "system" fn(Handle) -> i32> =
            library.get(b"WinMLEpEnsureReady\0")?;
        let path_size: Symbol<unsafe extern "system" fn(Handle, *mut usize) -> i32> =
            library.get(b"WinMLEpGetLibraryPathSize\0")?;
        let path_get: Symbol<
            unsafe extern "system" fn(Handle, usize, *mut c_char, *mut usize) -> i32,
        > = library.get(b"WinMLEpGetLibraryPath\0")?;
        let mut catalog = std::ptr::null_mut();
        check(create(&mut catalog), "Creating Windows ML provider catalog")?;
        ensure!(!catalog.is_null(), "Windows ML returned a null catalog");
        let result = (|| -> Result<()> {
            let mut providers = Vec::<Provider>::new();
            check(
                enumerate(
                    catalog,
                    collect,
                    (&mut providers as *mut Vec<Provider>).cast(),
                ),
                "Enumerating Windows ML providers",
            )?;
            for provider in providers {
                let prepared = (|| -> Result<()> {
                    // Windows handles acquisition and package dependency setup.
                    check(ready(provider.handle), "Preparing Windows ML provider")?;
                    let mut size = 0;
                    check(
                        path_size(provider.handle, &mut size),
                        "Reading provider path size",
                    )?;
                    ensure!(
                        size > 0 && size <= 1024 * 1024,
                        "Invalid Windows ML provider path size"
                    );
                    let mut path = vec![0u8; size];
                    check(
                        path_get(
                            provider.handle,
                            size,
                            path.as_mut_ptr().cast(),
                            std::ptr::null_mut(),
                        ),
                        "Reading provider path",
                    )?;
                    let path = CStr::from_bytes_until_nul(&path)?.to_str()?;
                    ensure!(
                        std::path::Path::new(path).is_absolute(),
                        "Provider path must be absolute"
                    );
                    env.register_ep_library(&provider.name, path)?;
                    Ok(())
                })();
                if let Err(error) = prepared {
                    tracing::warn!(provider = %provider.name, error = %format!("{error:#}"), "Windows ML provider unavailable");
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            release(catalog);
            return Err(error);
        }
        // Keep the successful catalog alive for the process. Provider objects
        // may own dynamic-dependency references required by plugin libraries.
        // REGISTERED ensures this retention happens only once.
    }
    *registered = true;
    Ok(())
}

/// Run in a fresh process; GPU mode forbids CPU fallback for the probe graph.
pub fn verify(require_gpu: bool, embedded: bool) -> Result<String> {
    let shared = select_api(embedded)?;
    ensure!(
        !require_gpu || shared,
        "Windows ML is required for GPU verification"
    );
    ensure!(!embedded || !shared, "Expected embedded CPU runtime");
    let env = builder()?.with_name("windows-ml-check").build()?;
    if require_gpu {
        register_providers(&env)?;
    }
    let devices: Vec<_> = env
        .devices()
        .map(|d| Ok(format!("{}: {:?}", d.ep()?, d.hardware_device().ty())))
        .collect::<ort::Result<_>>()?;
    // Always exercise the shared ORT ABI, even on GPU-less CI machines.
    {
        use ort::{
            memory::DeviceType,
            session::{Session, builder::AutoDevicePolicy},
            value::TensorRef,
        };
        let device = if require_gpu {
            Some(
                env.devices()
                    .find(|d| d.hardware_device().ty() == DeviceType::GPU)
                    .context("Windows ML reports no GPU provider")?,
            )
        } else {
            None
        };
        let build = || -> ort::Result<Session> {
            let mut builder = Session::builder(&env)?
                .with_auto_device(AutoDevicePolicy::PreferCPU)?
                .with_parallel_execution(false)?
                .with_memory_pattern(false)?;
            if let Some(device) = device {
                builder = builder
                    .with_disable_cpu_fallback()?
                    .with_devices([device], None)?;
            }
            builder.commit_from_memory(include_bytes!("windows_runtime/add.onnx"))
        };
        let mut session = build().context("Creating Windows ML inference probe")?;
        let a = ndarray::arr1(&[1.0f32, 2.0, 3.0]);
        let b = ndarray::arr1(&[4.0f32, 5.0, 6.0]);
        let result = session.run(ort::inputs!["A" => TensorRef::from_array_view(&a)?, "B" => TensorRef::from_array_view(&b)?])?;
        ensure!(
            result["Y"].try_extract_tensor::<f32>()?.1 == [5.0, 7.0, 9.0],
            "Incorrect Windows ML inference result"
        );
    }
    Ok(format!(
        "Windows ML: {}\nProviders: {}\nInference probe: {}\n",
        if shared {
            runtime()?.directory.display().to_string()
        } else {
            "embedded CPU".into()
        },
        devices.join(", "),
        if require_gpu {
            "GPU passed"
        } else {
            "CPU passed; GPU not tested"
        }
    ))
}
