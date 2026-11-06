use super::*;
use std::sync::atomic::AtomicUsize;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsWindowVisible, PostMessageW, WM_CLOSE,
};

struct StopOnDrop(Arc<AtomicBool>);

struct ObservedDesktop {
    desktop: WindowsUiApp,
    updates: Arc<AtomicUsize>,
}

impl eframe::App for ObservedDesktop {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.desktop.clear_color(visuals)
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.desktop.update(ctx, frame);
        self.updates.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn wait_until(description: &str, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out: {description}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn find_window(title: &str) -> HWND {
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
}

fn visible_window(title: &str) -> HWND {
    let mut window = std::ptr::null_mut();
    wait_until(title, || {
        window = find_window(title);
        !window.is_null() && unsafe { IsWindowVisible(window) } != 0
    });
    window
}

fn close_window(window: HWND) {
    assert_ne!(unsafe { PostMessageW(window, WM_CLOSE, 0, 0) }, 0);
    wait_until(
        "window should close or hide",
        || unsafe { IsWindowVisible(window) } == 0,
    );
}

fn working_set_bytes() -> usize {
    let mut memory: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    assert_ne!(
        unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut memory as *mut _ as _,
                std::mem::size_of_val(&memory) as u32,
            )
        },
        0
    );
    memory.WorkingSetSize
}

#[test]
#[ignore = "requires a Windows desktop and must run alone (one winit event loop per process)"]
fn desktop_reuses_one_event_loop_and_stays_stable_when_idle() {
    let running = Arc::new(AtomicBool::new(true));
    let _stop = StopOnDrop(running.clone());
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (resp_tx, _resp_rx) = crossbeam_channel::unbounded();
    let (config_tx, config_rx) = crossbeam_channel::unbounded();
    let desktop = WindowsUiApp::new(cmd_rx, resp_tx, running.clone(), config_tx);
    let updates = Arc::new(AtomicUsize::new(0));
    let desktop = ObservedDesktop {
        desktop,
        updates: updates.clone(),
    };
    let gui_running = running.clone();
    let gui_thread = std::thread::spawn(move || {
        let options = desktop_native_options();
        let result = eframe::run_native(
            "Copy Time Tray - Snippet",
            options,
            Box::new(|_| Ok(Box::new(desktop))),
        )
        .map_err(|error| error.to_string());
        gui_running.store(false, Ordering::SeqCst);
        eprintln!("GUI result: {result:?}");
        result
    });
    let tray_running = running.clone();
    let tray_cmd_tx = cmd_tx.clone();
    let tray_thread = std::thread::spawn(move || {
        super::main(
            tray_cmd_tx,
            tray_running,
            config_rx,
            Arc::new(AtomicBool::new(true)),
        )
    });

    wait_until("desktop host should initialize", || {
        !find_window("Copy Time Tray").is_null()
    });
    wait_until("first GUI update", || updates.load(Ordering::SeqCst) > 0);
    let initial_updates = updates.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        updates.load(Ordering::SeqCst) >= initial_updates + 3,
        "host must keep processing commands while all tool windows are closed"
    );
    assert!(find_window("代码片段").is_null());

    for cycle in 0..5 {
        cmd_tx.send(UiCommand::Show).unwrap();
        let snippets = visible_window("代码片段");
        cmd_tx.send(UiCommand::ShowJson).unwrap();
        let json = visible_window("JSON 格式化工具");
        cmd_tx.send(UiCommand::ShowConfig).unwrap();
        let settings = visible_window("Copy Time Tray - 配置");
        eprintln!("cycle {cycle}: closing snippets");
        close_window(snippets);
        assert_ne!(unsafe { IsWindowVisible(json) }, 0);
        assert_ne!(unsafe { IsWindowVisible(settings) }, 0);
        eprintln!("cycle {cycle}: closing JSON");
        close_window(json);
        eprintln!("cycle {cycle}: closing settings");
        close_window(settings);
        assert!(
            !gui_thread.is_finished(),
            "closing windows must not end the GUI loop"
        );
        eprintln!(
            "window cycle {cycle}: working set bytes={}",
            working_set_bytes()
        );
    }

    std::thread::sleep(Duration::from_secs(1));
    let baseline = working_set_bytes();
    assert!(baseline > 0, "memory measurement must be available");
    for sample in 0..30 {
        std::thread::sleep(Duration::from_secs(1));
        let memory = working_set_bytes();
        eprintln!("idle sample {sample}: working set bytes={memory}");
        assert!(
            memory <= baseline + 16 * 1024 * 1024,
            "idle memory should stabilize"
        );
    }

    cmd_tx.send(UiCommand::ShowJson).unwrap();
    close_window(visible_window("JSON 格式化工具"));

    running.store(false, Ordering::SeqCst);
    tray_thread.join().unwrap();
    assert_eq!(gui_thread.join().unwrap(), Ok(()));
}
