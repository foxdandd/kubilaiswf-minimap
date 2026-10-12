mod archive;
mod config;
mod formats;
mod game;
mod maptiles;
mod minimap;
mod reader;

use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp};
use eldenring::fd4::FD4TaskData;
use fromsoftware_shared::SharedTaskImpExt;
use hudhook::Hudhook;
use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::windows::Win32::Foundation::HMODULE;
use hudhook::windows::Win32::System::LibraryLoader::GetModuleFileNameW;

/// # Safety
/// Called by the Windows loader when me3 loads this DLL. Do not call it yourself.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(module: usize, reason: u32, _reserved: usize) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        std::thread::spawn(move || init(module));
    }
    1
}

fn init(module: usize) {
    let Ok(task) = CSTaskImp::wait_for_instance(Duration::MAX) else {
        return;
    };
    let handle = task.run_recurring(
        |_: &FD4TaskData| game::update(),
        // At the start of the frame, outside the game's physics and character update steps.
        CSTaskGroupIndex::FrameBegin,
    );
    // Dropping the handle unregisters the task, and it should run for the rest of the session.
    std::mem::forget(handle);

    let Some(game_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(ToOwned::to_owned))
    else {
        return;
    };
    let dll = dll_path(module);
    let config = config::Config::load(&dll.with_file_name("minimap.ini"));
    let _ = Hudhook::builder()
        .with::<ImguiDx12Hooks>(minimap::Minimap::new(&game_dir, config))
        .build()
        .apply();
}

fn dll_path(module: usize) -> PathBuf {
    let mut buffer = [0u16; 1024];
    // SAFETY: `module` is this DLL's handle as passed to DllMain.
    let len =
        unsafe { GetModuleFileNameW(Some(HMODULE(module as *mut c_void)), &mut buffer) } as usize;
    PathBuf::from(std::ffi::OsString::from_wide(&buffer[..len]))
}
