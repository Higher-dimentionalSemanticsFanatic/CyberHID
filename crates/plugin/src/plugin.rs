//! The RED4ext entry points, the XInput hooks and the device threads.
//!
//! RED4ext loads the DLL from `red4ext/plugins/cyberhid/` and calls
//! `Supports`, `Query` and `Main`. `Main` hooks XInputGetState (and
//! XInputGetStateEx, XInputGetCapabilities) in every XInput DLL through
//! RED4ext's hooking API, then a background thread watches the bindings
//! (`config.json`, saved by the controls window in Cyber Engine Tweaks),
//! opens the connected controllers and writes the state file the window
//! shows. One thread per open controller waits for its input reports. The
//! hooks add the bound inputs to the game's reads of the bound gamepad
//! slot; inputs bound to game keys (F13 to F24) are pressed as keyboard
//! keys while a game window is in front.

use std::collections::HashSet;
use std::ffi::{CString, c_void};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cyberhid_core::config::{CONFIG_FILE, Config};
use cyberhid_core::{self as hm, Active, Binding, Live, Moves, Pad, Status, StatusDevice, kind, win};
use windows_sys::Win32::Foundation::{ERROR_DEVICE_NOT_CONNECTED, ERROR_SUCCESS, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleFileNameW, GetModuleHandleExW,
    GetModuleHandleW, GetProcAddress, LoadLibraryW,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput};
use windows_sys::Win32::UI::Input::XboxController::{XINPUT_CAPABILITIES, XINPUT_GAMEPAD, XINPUT_STATE};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

// ---- RED4ext ABI (RED4ext.SDK, include/RED4ext/Api/v1) -----------------------

#[repr(C)]
pub struct SemVer {
    major: u8,
    minor: u16,
    patch: u32,
    prerelease_type: u32,
    prerelease_number: u32,
}

#[repr(C)]
pub struct FileVer {
    major: u16,
    minor: u16,
    build: u16,
    revision: u16,
}

#[repr(C)]
pub struct PluginInfo {
    sdk: SemVer,
    name: *const u16,
    author: *const u16,
    version: SemVer,
    runtime: FileVer,
}

/// Trace, Debug, Info, Warn, Error and Critical, each as plain, printf,
/// wide and wide printf.
#[repr(C)]
pub struct Logger {
    fns: [*const c_void; 24],
}

#[repr(C)]
pub struct Hooking {
    attach: unsafe extern "C" fn(HMODULE, *mut c_void, *mut c_void, *mut *mut c_void) -> bool,
    detach: unsafe extern "C" fn(HMODULE, *mut c_void) -> bool,
}

#[repr(C)]
pub struct Sdk {
    runtime: *const SemVer,
    logger: *const Logger,
    hooking: *const Hooking,
    game_states: *const c_void,
    scripts: *const c_void,
}

/// `RED4EXT_API_VERSION_1_COMPAT_0`: API v1, accepted by older loaders too.
const API_VERSION: u32 = 0;

const fn wide<const N: usize>(s: &str) -> [u16; N] {
    let b = s.as_bytes();
    let mut out = [0u16; N];
    let mut i = 0;
    while i < b.len() {
        out[i] = b[i] as u16;
        i += 1;
    }
    out
}

static NAME: [u16; hm::PLUGIN_NAME.len() + 1] = wide(hm::PLUGIN_NAME);
const AUTHOR_STR: &str = "CyberHID";
static AUTHOR: [u16; AUTHOR_STR.len() + 1] = wide(AUTHOR_STR);
const VERSION: &str = hm::PLUGIN_VERSION;

const fn version_part(n: usize) -> u32 {
    let s = VERSION.as_bytes();
    let (mut i, mut part, mut v) = (0, 0, 0u32);
    while i < s.len() {
        let c = s[i];
        if c == b'.' {
            if part == n {
                return v;
            }
            part += 1;
            v = 0;
        } else if c.is_ascii_digit() {
            v = v * 10 + (c - b'0') as u32;
        } else {
            break;
        }
        i += 1;
    }
    if part == n { v } else { 0 }
}

/// RED4ext asks which API the plugin speaks.
#[unsafe(no_mangle)]
pub extern "C" fn Supports() -> u32 {
    API_VERSION
}

/// RED4ext asks for the plugin's name, author and versions.
///
/// # Safety
/// `info` points to RED4ext's `PluginInfo`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Query(info: *mut PluginInfo) {
    if info.is_null() {
        return;
    }
    let semver = |major: u32, minor: u32, patch: u32| SemVer {
        major: major as u8,
        minor: minor as u16,
        patch,
        prerelease_type: 0,
        prerelease_number: 0,
    };
    unsafe {
        // RED4EXT_V1_SDK_VERSION_1_0_0_COMPAT_0_5_0 and RED4EXT_V1_RUNTIME_VERSION_INDEPENDENT:
        // no game offsets are used, so any game version.
        (*info).sdk = semver(0, 5, 0);
        (*info).name = NAME.as_ptr();
        (*info).author = AUTHOR.as_ptr();
        (*info).version = semver(version_part(0), version_part(1), version_part(2));
        (*info).runtime = FileVer { major: u16::MAX, minor: u16::MAX, build: u16::MAX, revision: u16::MAX };
    }
}

/// RED4ext loads (reason 0) or unloads (1) the plugin.
///
/// # Safety
/// `sdk` points to RED4ext's `Sdk`, valid while the plugin is loaded.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Main(handle: HMODULE, reason: u8, sdk: *const Sdk) -> bool {
    if sdk.is_null() {
        return false;
    }
    let sdk = unsafe { &*sdk };
    match reason {
        0 => unsafe { load(handle, sdk) },
        _ => unsafe { unload(handle, sdk) },
    }
    true
}

// ---- state shared by the hooks and the threads -------------------------------

struct Connected {
    info: win::Info,
    connected: bool,
}

#[derive(Default)]
struct Engine {
    /// `config.json` (defaults until saved).
    config: Config,
    bindings: Vec<Binding>,
    config_saved: bool,
    config_error: Option<String>,
    config_warnings: Vec<String>,
    /// Controllers opened since the game started; a slot is reused when the
    /// same controller comes back.
    devices: Vec<Connected>,
    live: Vec<Live>,
    moves: Vec<Moves>,
    active: Vec<Active>,
    /// Every game controller seen by the last scan.
    seen: Vec<win::Info>,
    /// A real gamepad answered the last read of the bound slot.
    slot_pad: bool,
    /// An active binding drives a game key.
    has_keys: bool,
    /// Game keys this plugin holds down (bit 0: F13).
    keys_down: u16,
    /// Per XInput slot: the last pad returned and how often it changed.
    last: [Pad; 4],
    changes: [u32; 4],
}

fn id(i: &win::Info) -> String {
    hm::device_id(i.vid, i.pid, i.instance)
}

impl Engine {
    /// The open controller with this [`hm::device_id`].
    fn controller(&self, device: &str) -> Option<usize> {
        self.devices.iter().position(|d| d.connected && id(&d.info) == device)
    }

    /// Match every binding to an open controller and input.
    fn rebuild(&mut self) {
        let mut active = Vec::new();
        for b in &self.bindings {
            let Some(i) = self.controller(&b.device) else { continue };
            if let Some(slot) = hm::resolve(&b.input, &self.devices[i].info.layout) {
                active.push(Active { binding: b.clone(), device: i, slot });
            }
        }
        self.has_keys = active.iter().any(|a| a.binding.target.key().is_some());
        self.active = active;
    }

    /// Bound controllers that are not open, as `name (id)`, one entry per
    /// controller.
    fn missing(&self) -> Vec<String> {
        let mut ids: Vec<&str> = Vec::new();
        for b in self.config.bindings.iter().filter(|b| self.controller(&b.device).is_none()) {
            if !ids.contains(&b.device.as_str()) {
                ids.push(&b.device);
            }
        }
        ids.into_iter()
            .map(|id| {
                let name = self.config.bindings.iter().find(|b| b.device == id && !b.device_name.is_empty());
                name.map_or_else(|| id.to_string(), |b| format!("{} ({id})", b.device_name))
            })
            .collect()
    }

    fn has_missing(&self) -> bool {
        self.config.bindings.iter().any(|b| self.controller(&b.device).is_none())
    }

    /// XInput user index of the bound slot.
    fn user(&self) -> u32 {
        self.config.slot.clamp(1, 4) - 1
    }
}

static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();

fn engine() -> MutexGuard<'static, Engine> {
    ENGINE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

static GET_STATE_CALLS: AtomicU64 = AtomicU64::new(0);
static MERGED_READS: AtomicU64 = AtomicU64::new(0);
static EMULATED_READS: AtomicU64 = AtomicU64::new(0);
/// Inputs moved (`Moves`), so the state file follows soon after.
static MOVES: AtomicU64 = AtomicU64::new(0);
static STOPPING: AtomicBool = AtomicBool::new(false);
static HOOKS: Mutex<Vec<(String, usize)>> = Mutex::new(Vec::new());
static HOOK_ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

// ---- XInput hooks --------------------------------------------------------------

type GetState = unsafe extern "system" fn(u32, *mut XINPUT_STATE) -> u32;
type GetCaps = unsafe extern "system" fn(u32, u32, *mut XINPUT_CAPABILITIES) -> u32;

/// Every XInput DLL a game may use; each one the game loads gets hooked.
const MODULES: [&str; 5] = ["xinput1_4.dll", "xinput1_3.dll", "xinput9_1_0.dll", "xinput1_2.dll", "xinput1_1.dll"];

#[allow(clippy::declare_interior_mutable_const)]
const NULL: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIG_STATE: [AtomicPtr<c_void>; 5] = [NULL; 5];
static ORIG_STATE_EX: [AtomicPtr<c_void>; 5] = [NULL; 5];
static ORIG_CAPS: [AtomicPtr<c_void>; 5] = [NULL; 5];

unsafe extern "system" fn get_state<const I: usize>(user: u32, state: *mut XINPUT_STATE) -> u32 {
    unsafe { on_state(&ORIG_STATE[I], user, state) }
}

unsafe extern "system" fn get_state_ex<const I: usize>(user: u32, state: *mut XINPUT_STATE) -> u32 {
    unsafe { on_state(&ORIG_STATE_EX[I], user, state) }
}

unsafe extern "system" fn get_caps<const I: usize>(user: u32, flags: u32, caps: *mut XINPUT_CAPABILITIES) -> u32 {
    unsafe { on_caps(&ORIG_CAPS[I], user, flags, caps) }
}

const STATE_DETOURS: [GetState; 5] = [get_state::<0>, get_state::<1>, get_state::<2>, get_state::<3>, get_state::<4>];
const STATE_EX_DETOURS: [GetState; 5] =
    [get_state_ex::<0>, get_state_ex::<1>, get_state_ex::<2>, get_state_ex::<3>, get_state_ex::<4>];
const CAPS_DETOURS: [GetCaps; 5] = [get_caps::<0>, get_caps::<1>, get_caps::<2>, get_caps::<3>, get_caps::<4>];

fn to_pad(g: &XINPUT_GAMEPAD) -> Pad {
    Pad {
        buttons: g.wButtons,
        left_trigger: g.bLeftTrigger,
        right_trigger: g.bRightTrigger,
        lx: g.sThumbLX,
        ly: g.sThumbLY,
        rx: g.sThumbRX,
        ry: g.sThumbRY,
    }
}

/// Whether the plugin answers for `user` when no real gamepad is there.
fn emulating(e: &Engine, user: u32) -> bool {
    e.config.enabled && e.config.report_pad && user == e.user() && !e.active.is_empty()
}

unsafe fn on_state(orig: &AtomicPtr<c_void>, user: u32, state: *mut XINPUT_STATE) -> u32 {
    GET_STATE_CALLS.fetch_add(1, Ordering::Relaxed);
    let f = orig.load(Ordering::Acquire);
    let r = if f.is_null() {
        ERROR_DEVICE_NOT_CONNECTED
    } else {
        unsafe { std::mem::transmute::<*mut c_void, GetState>(f)(user, state) }
    };
    if state.is_null() || STOPPING.load(Ordering::Relaxed) {
        return r;
    }
    let mut e = engine();
    if user != e.user() {
        return r;
    }
    e.slot_pad = r == ERROR_SUCCESS;
    if !e.config.enabled {
        return r;
    }
    let (real, packet) = if r == ERROR_SUCCESS {
        let s = unsafe { &*state };
        (to_pad(&s.Gamepad), s.dwPacketNumber)
    } else if r == ERROR_DEVICE_NOT_CONNECTED && emulating(&e, user) {
        EMULATED_READS.fetch_add(1, Ordering::Relaxed);
        (Pad::default(), 0)
    } else {
        return r;
    };
    let c = hm::contribution(&e.active, &e.live);
    if !c.is_neutral() {
        MERGED_READS.fetch_add(1, Ordering::Relaxed);
    }
    let pad = c.apply(real);
    // Games skip a read whose packet number did not change, so count our
    // own changes into it.
    let slot = user as usize;
    if pad != e.last[slot] {
        e.last[slot] = pad;
        e.changes[slot] = e.changes[slot].wrapping_add(1);
    }
    let s = unsafe { &mut *state };
    s.dwPacketNumber = packet.wrapping_add(e.changes[slot]);
    s.Gamepad = XINPUT_GAMEPAD {
        wButtons: pad.buttons,
        bLeftTrigger: pad.left_trigger,
        bRightTrigger: pad.right_trigger,
        sThumbLX: pad.lx,
        sThumbLY: pad.ly,
        sThumbRX: pad.rx,
        sThumbRY: pad.ry,
    };
    ERROR_SUCCESS
}

unsafe fn on_caps(orig: &AtomicPtr<c_void>, user: u32, flags: u32, caps: *mut XINPUT_CAPABILITIES) -> u32 {
    let f = orig.load(Ordering::Acquire);
    let r = if f.is_null() {
        ERROR_DEVICE_NOT_CONNECTED
    } else {
        unsafe { std::mem::transmute::<*mut c_void, GetCaps>(f)(user, flags, caps) }
    };
    if r != ERROR_DEVICE_NOT_CONNECTED || caps.is_null() || STOPPING.load(Ordering::Relaxed) || !emulating(&engine(), user) {
        return r;
    }
    let c = unsafe { &mut *caps };
    c.Type = 1; // XINPUT_DEVTYPE_GAMEPAD
    c.SubType = 1; // XINPUT_DEVSUBTYPE_GAMEPAD
    c.Flags = 0;
    c.Gamepad = XINPUT_GAMEPAD {
        wButtons: 0xF3FF,
        bLeftTrigger: 255,
        bRightTrigger: 255,
        sThumbLX: -64,
        sThumbLY: -64,
        sThumbRX: -64,
        sThumbRY: -64,
    };
    c.Vibration.wLeftMotorSpeed = 0;
    c.Vibration.wRightMotorSpeed = 0;
    ERROR_SUCCESS
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn log(sdk: &Sdk, handle: HMODULE, level: usize, msg: &str) {
    // Info 8, Warn 12, Error 16: the plain (char*) variants.
    let Ok(text) = CString::new(msg.replace('\0', " ")) else { return };
    if sdk.logger.is_null() {
        return;
    }
    let f = unsafe { (*sdk.logger).fns[level] };
    if f.is_null() {
        return;
    }
    let f: unsafe extern "C" fn(HMODULE, *const u8) = unsafe { std::mem::transmute(f) };
    unsafe { f(handle, text.as_ptr().cast()) };
}

const INFO: usize = 8;
const WARN: usize = 12;

/// Hook the XInput functions of every XInput DLL, loading the ones the game
/// hasn't loaded yet so a later load by the game gets the hooked copy.
unsafe fn hook_xinput(handle: HMODULE, sdk: &Sdk) {
    if sdk.hooking.is_null() {
        HOOK_ERRORS.lock().unwrap_or_else(|e| e.into_inner()).push("RED4ext gave no hooking API".into());
        return;
    }
    let hooking = unsafe { &*sdk.hooking };
    let mut done: HashSet<usize> = HashSet::new();
    for (i, name) in MODULES.iter().enumerate() {
        let w = wide_z(name);
        let mut m = unsafe { GetModuleHandleW(w.as_ptr()) };
        if m.is_null() {
            m = unsafe { LoadLibraryW(w.as_ptr()) };
        }
        if m.is_null() {
            continue;
        }
        let targets: [(&str, *const u8, *mut c_void, &AtomicPtr<c_void>); 3] = [
            ("XInputGetState", c"XInputGetState".as_ptr().cast(), STATE_DETOURS[i] as *mut c_void, &ORIG_STATE[i]),
            // Ordinal 100: XInputGetStateEx, which adds the guide button.
            ("XInputGetStateEx", 100 as *const u8, STATE_EX_DETOURS[i] as *mut c_void, &ORIG_STATE_EX[i]),
            ("XInputGetCapabilities", c"XInputGetCapabilities".as_ptr().cast(), CAPS_DETOURS[i] as *mut c_void, &ORIG_CAPS[i]),
        ];
        for (fname, proc, detour, orig) in targets {
            let Some(target) = (unsafe { GetProcAddress(m, proc) }) else { continue };
            let target = target as *mut c_void;
            // A forwarded export is the same function: hook it once.
            if !done.insert(target as usize) {
                continue;
            }
            let label = format!("{name}!{fname}");
            if unsafe { (hooking.attach)(handle, target, detour, orig.as_ptr()) } {
                HOOKS.lock().unwrap_or_else(|e| e.into_inner()).push((label.clone(), target as usize));
                log(sdk, handle, INFO, &format!("hooked {label}"));
            } else {
                HOOK_ERRORS.lock().unwrap_or_else(|e| e.into_inner()).push(format!("{label}: RED4ext could not attach the hook"));
                log(sdk, handle, WARN, &format!("could not hook {label}"));
            }
        }
    }
}

fn module_dir(handle: HMODULE) -> Option<PathBuf> {
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetModuleFileNameW(handle, buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return None;
    }
    let path = PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    path.parent().map(Path::to_path_buf)
}

unsafe fn load(handle: HMODULE, sdk: &Sdk) {
    // Device threads outlive RED4ext's unload at game exit: keep the DLL
    // mapped until the process ends.
    let mut pinned: HMODULE = std::ptr::null_mut();
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            Main as *const u16,
            &mut pinned,
        )
    };
    let Some(plugin_dir) = module_dir(handle) else {
        log(sdk, handle, WARN, "plugin folder not found; nothing bound");
        return;
    };
    {
        let dir = data_dir(&plugin_dir);
        let mut e = engine();
        load_config(&mut e, &dir.join(CONFIG_FILE));
        let bound = match (&e.config_error, e.config_saved) {
            (Some(err), _) => err.clone(),
            (None, true) => format!("{} binding(s) in {}", e.config.bindings.len(), dir.join(CONFIG_FILE).display()),
            (None, false) => format!("no {} in {}", CONFIG_FILE, dir.display()),
        };
        drop(e);
        log(sdk, handle, INFO, &format!("{} {VERSION}: {bound}", hm::PLUGIN_NAME));
    }
    unsafe { hook_xinput(handle, sdk) };
    let started = unix_now();
    let _ = std::thread::Builder::new().name("cyberhid-monitor".into()).spawn(move || monitor(plugin_dir, started));
}

unsafe fn unload(handle: HMODULE, sdk: &Sdk) {
    STOPPING.store(true, Ordering::Relaxed);
    let held = std::mem::take(&mut engine().keys_down);
    send_keys(0, held);
    if sdk.hooking.is_null() {
        return;
    }
    let hooks = std::mem::take(&mut *HOOKS.lock().unwrap_or_else(|e| e.into_inner()));
    for (_, target) in hooks {
        unsafe { ((*sdk.hooking).detach)(handle, target as *mut c_void) };
    }
}

// ---- bindings, devices and status ----------------------------------------------

/// Where `config.json` and the state file live: the controls window's
/// folder under the game root (three levels above this plugin's folder),
/// else this plugin's folder.
fn data_dir(plugin_dir: &Path) -> PathBuf {
    if let Some(game) = plugin_dir.ancestors().nth(3) {
        let cet = hm::CET_DIR.split('/').fold(game.to_path_buf(), |p, c| p.join(c));
        if cet.is_dir() {
            return cet;
        }
    }
    plugin_dir.to_path_buf()
}

/// Read `config.json`; false when it could not be read and should be
/// tried again (the window may be writing it).
fn load_config(e: &mut Engine, path: &Path) -> bool {
    let ok = match std::fs::read(path) {
        Ok(bytes) => match Config::parse(&String::from_utf8_lossy(&bytes)) {
            Ok((c, warnings)) => {
                e.config = c;
                e.config_saved = true;
                e.config_error = None;
                e.config_warnings = warnings;
                true
            }
            // Keep the previous bindings until the file reads again.
            Err(err) => {
                e.config_error = Some(err);
                false
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            e.config = Config::default();
            e.config_saved = false;
            e.config_error = None;
            e.config_warnings.clear();
            true
        }
        Err(err) => {
            e.config_error = Some(format!("{CONFIG_FILE}: {err}"));
            false
        }
    };
    e.bindings = e.config.bindings();
    e.rebuild();
    ok
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn unix_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Open the connected controllers not yet open: all but XInput gamepads,
/// so the controls window shows their inputs moving before they are bound.
fn scan() {
    let found = win::list();
    let mut e = engine();
    e.seen = found.clone();
    let mut opened = Vec::new();
    for info in &found {
        if info.xinput || e.devices.iter().any(|c| c.connected && c.info.path == info.path) {
            continue;
        }
        let Some(reader) = win::Reader::open(&info.path) else { continue };
        let mut info = info.clone();
        info.layout = reader.layout();
        let idle = reader.idle();
        let slot = match e.devices.iter().position(|c| c.info.path == info.path) {
            Some(i) => {
                e.devices[i] = Connected { info, connected: true };
                e.live[i] = idle;
                i
            }
            None => {
                e.devices.push(Connected { info, connected: true });
                e.live.push(idle);
                e.moves.push(Moves::default());
                e.devices.len() - 1
            }
        };
        opened.push((slot, reader));
    }
    // A bound controller may have come back.
    e.rebuild();
    drop(e);
    for (slot, reader) in opened {
        let _ = std::thread::Builder::new().name(format!("cyberhid-{slot}")).spawn(move || read_device(slot, reader));
    }
}

fn read_device(slot: usize, mut reader: win::Reader) {
    let mut live = reader.idle();
    while !STOPPING.load(Ordering::Relaxed) && reader.read(&mut live) {
        let mut e = engine();
        if let Some(l) = e.live.get_mut(slot) {
            l.clone_from(&live);
        }
        if e.moves.get_mut(slot).is_some_and(|m| m.update(&live)) {
            MOVES.fetch_add(1, Ordering::Relaxed);
        }
        let keys = e.has_keys || e.keys_down != 0;
        drop(e);
        if keys {
            sync_keys();
        }
    }
    let mut e = engine();
    if let Some(c) = e.devices.get_mut(slot) {
        c.connected = false;
    }
    if let Some(l) = e.live.get_mut(slot) {
        *l = reader.idle();
    }
    e.rebuild();
    drop(e);
    sync_keys();
}

// ---- game keys -------------------------------------------------------------------

/// A window of this process (the game) is in front, so keys sent now reach
/// the game and no other program.
fn game_in_front() -> bool {
    let w = unsafe { GetForegroundWindow() };
    if w.is_null() {
        return false;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(w, &mut pid) };
    pid == unsafe { GetCurrentProcessId() }
}

/// Press and release game keys to match the bound inputs. Keys are held
/// only while the game is in front; they are released when it isn't.
fn sync_keys() {
    let (press, release) = {
        let mut e = engine();
        if !e.has_keys && e.keys_down == 0 {
            return;
        }
        let want = if e.config.enabled && e.has_keys && game_in_front() { hm::contribution(&e.active, &e.live).keys } else { 0 };
        let down = e.keys_down;
        e.keys_down = want;
        (want & !down, down & !want)
    };
    send_keys(press, release);
}

fn send_keys(press: u16, release: u16) {
    let inputs: Vec<INPUT> = hm::GAME_KEYS
        .iter()
        .enumerate()
        .filter(|(i, _)| (press | release) & (1 << i) != 0)
        .map(|(i, (vk, scan))| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *vk,
                    wScan: *scan,
                    dwFlags: if release & (1 << i) != 0 { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect();
    if !inputs.is_empty() {
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    }
}

fn status(started: u64) -> Status {
    let e = engine();
    let devices = e
        .seen
        .iter()
        .map(|i| {
            let open = e.devices.iter().position(|c| c.connected && c.info.path == i.path);
            let k = kind::classify(i.xinput, i.top, &i.layout.axes, i.layout.buttons, &i.name);
            let mut d = StatusDevice {
                id: id(i),
                name: i.name.clone(),
                vid: i.vid,
                pid: i.pid,
                kind: k,
                kind_label: k.label().into(),
                xinput: i.xinput,
                connected: open.is_some(),
                recent_inputs: open.and_then(|n| e.moves.get(n)).map(|m| m.recent.clone()).unwrap_or_default(),
                bound: open.is_some_and(|n| e.active.iter().any(|a| a.device == n)),
                ..StatusDevice::default()
            };
            let layout = open.map_or(&i.layout, |n| &e.devices[n].info.layout);
            d.inputs(layout, open.and_then(|n| e.live.get(n)));
            d
        })
        .collect();
    Status {
        plugin_version: VERSION.into(),
        wine: wine_version(),
        started,
        updated: unix_millis(),
        hooks: HOOKS.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(l, _)| l.clone()).collect(),
        hook_errors: HOOK_ERRORS.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        get_state_calls: GET_STATE_CALLS.load(Ordering::Relaxed),
        merged_reads: MERGED_READS.load(Ordering::Relaxed),
        emulated_reads: EMULATED_READS.load(Ordering::Relaxed),
        config_error: e.config_error.clone(),
        config_warnings: e.config_warnings.clone(),
        config_saved: e.config_saved,
        enabled: e.config.enabled,
        slot: e.config.slot,
        slot_pad: e.slot_pad,
        bindings: e.config.bindings.len(),
        active_bindings: e.active.len(),
        devices,
        missing: e.missing(),
        output: hm::contribution(&e.active, &e.live).into(),
        keys_held: e.keys_down,
    }
}

/// `wine_get_version` from ntdll, present only under Wine and Proton.
fn wine_version() -> Option<String> {
    static VERSION: OnceLock<Option<String>> = OnceLock::new();
    VERSION
        .get_or_init(|| {
            let ntdll = unsafe { GetModuleHandleW(wide_z("ntdll.dll").as_ptr()) };
            if ntdll.is_null() {
                return None;
            }
            let f = unsafe { GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()) }?;
            let f: unsafe extern "C" fn() -> *const std::ffi::c_char = unsafe { std::mem::transmute(f) };
            let p = unsafe { f() };
            (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned())
        })
        .clone()
}

fn write_state(dir: &Path, started: u64) {
    let Ok(text) = serde_json::to_vec(&status(started)) else { return };
    let tmp = dir.join(format!("{}.tmp", hm::STATE_FILE));
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(hm::STATE_FILE));
    }
}

/// The controls window rewrote its heartbeat file in the last 3 seconds.
fn window_open(dir: &Path) -> bool {
    modified(&dir.join(hm::UI_FILE)).and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|age| age < Duration::from_secs(3))
}

/// Reload the bindings when the window saves them, open controllers as
/// they connect, and write the state file: 20 times a second while the
/// window is open, else every 5 seconds and soon after an input moves.
fn monitor(plugin_dir: PathBuf, started: u64) {
    let mut dir = data_dir(&plugin_dir);
    // Read again at the first check, in case the read at load failed.
    let mut config_time = Some(UNIX_EPOCH);
    let mut last_check: Option<Instant> = None;
    let mut last_scan: Option<Instant> = None;
    let mut last_state: Option<Instant> = None;
    let mut moves = MOVES.load(Ordering::Relaxed);
    let (mut open, mut missing) = (false, false);
    while !STOPPING.load(Ordering::Relaxed) {
        let mut changed = false;
        if last_check.is_none_or(|c| c.elapsed() >= Duration::from_millis(500)) {
            last_check = Some(Instant::now());
            // CET's folder may appear after the game started.
            let d = data_dir(&plugin_dir);
            let path = d.join(CONFIG_FILE);
            let t = modified(&path);
            if d != dir || t != config_time {
                dir = d;
                changed = true;
                // A failed read is tried again on the next check.
                config_time = if load_config(&mut engine(), &path) { t } else { None };
            }
            open = window_open(&dir);
            missing = engine().has_missing();
        }
        // Releases held keys when the game loses focus or a binding goes.
        sync_keys();
        let due = last_scan.is_none_or(|s| s.elapsed() >= Duration::from_secs(if missing { 3 } else { 30 }));
        if changed || due {
            scan();
            last_scan = Some(Instant::now());
        }
        let m = MOVES.load(Ordering::Relaxed);
        let every = Duration::from_millis(if open { 50 } else { 5000 });
        if last_state.is_none_or(|s| s.elapsed() >= every) || changed || m != moves {
            moves = m;
            write_state(&dir, started);
            last_state = Some(Instant::now());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
