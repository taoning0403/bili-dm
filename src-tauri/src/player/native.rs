//! Dynamic binding to mpv's stable client.h API. The library outlives its handle.
use crate::core::error::{AppError, AppResult};
use libloading::Library;
use serde_json::Value;
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    path::Path,
};

#[repr(C)]
union NodeData {
    string: *mut c_char,
    flag: c_int,
    integer: i64,
    double: f64,
    list: *mut NodeList,
    bytes: *mut ByteArray,
}
#[repr(C)]
struct ByteArray {
    data: *mut c_void,
    size: usize,
}
#[repr(C)]
struct Node {
    data: NodeData,
    format: c_int,
}
#[repr(C)]
struct NodeList {
    count: c_int,
    values: *mut Node,
    keys: *mut *mut c_char,
}
#[repr(C)]
struct Event {
    id: c_int,
    error: c_int,
    userdata: u64,
    data: *mut c_void,
}
#[repr(C)]
struct EndFile {
    reason: c_int,
    error: c_int,
}

struct Api {
    create: unsafe extern "C" fn() -> *mut c_void,
    initialize: unsafe extern "C" fn(*mut c_void) -> c_int,
    destroy: unsafe extern "C" fn(*mut c_void),
    option: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(*mut c_void, *const *const c_char) -> c_int,
    command_ret: unsafe extern "C" fn(*mut c_void, *const *const c_char, *mut Node) -> c_int,
    get: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int,
    free_node: unsafe extern "C" fn(*mut Node),
    error: unsafe extern "C" fn(c_int) -> *const c_char,
    event: unsafe extern "C" fn(*mut c_void, f64) -> *const Event,
    _library: Library,
}

pub struct Client {
    api: Api,
    handle: *mut c_void,
    pub loaded: bool,
    pub last_error: Option<String>,
    pub frame_revision: u64,
}
// mpv permits calls from any thread. The owning Mutex ensures a single caller
// and prevents destruction while a property or event pointer is being read.
unsafe impl Send for Client {}

fn cstring(value: &str) -> AppResult<CString> {
    CString::new(value).map_err(|_| AppError::new("PLAYER_ARGUMENT", "播放器参数含有空字符。"))
}

impl Client {
    pub fn open(path: &Path, wid: Option<i64>) -> AppResult<Self> {
        // SAFETY: all symbols and repr(C) layouts follow libmpv client API 2.
        // Api retains the Library until after terminate_destroy.
        let api = unsafe {
            let lib = Library::new(path).map_err(|e| {
                AppError::new(
                    "PLAYER_RUNTIME",
                    format!("无法加载 libmpv：{e}。请运行 npm run player:setup。"),
                )
            })?;
            macro_rules! symbol {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes())
                        .map_err(|e| AppError::new("PLAYER_RUNTIME", e.to_string()))?
                };
            }
            Api {
                create: symbol!("mpv_create"),
                initialize: symbol!("mpv_initialize"),
                destroy: symbol!("mpv_terminate_destroy"),
                option: symbol!("mpv_set_option_string"),
                command: symbol!("mpv_command"),
                command_ret: symbol!("mpv_command_ret"),
                get: symbol!("mpv_get_property"),
                free_node: symbol!("mpv_free_node_contents"),
                error: symbol!("mpv_error_string"),
                event: symbol!("mpv_wait_event"),
                _library: lib,
            }
        };
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err(AppError::new("PLAYER_INIT", "无法创建 libmpv。"));
        }
        let client = Self {
            api,
            handle,
            loaded: false,
            last_error: None,
            frame_revision: 0,
        };
        for (key, value) in [
            ("config", "no"),
            ("terminal", "no"),
            ("idle", "yes"),
            ("keep-open", "always"),
            ("osc", "no"),
            // Bili DM owns controls, profiles and key bindings. Do not start
            // mpv's LuaJIT scripts in a hardened host process.
            ("load-scripts", "no"),
            ("load-stats-overlay", "no"),
            ("load-console", "no"),
            ("load-commands", "no"),
            ("load-auto-profiles", "no"),
            ("load-select", "no"),
            ("load-context-menu", "no"),
            ("load-positioning", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("input-media-keys", "no"),
            ("ytdl", "no"),
            ("hwdec", "auto-safe"),
            ("cache", "yes"),
            ("cache-secs", "30"),
            ("demuxer-max-bytes", "128MiB"),
            ("demuxer-max-back-bytes", "32MiB"),
            ("network-timeout", "30"),
            ("sub-auto", "fuzzy"),
            ("sub-use-margins", "no"),
            ("sub-clear-on-seek", "yes"),
            ("slang", "zh,chi,zho,eng,en"),
        ] {
            client.option(key, value)?;
        }
        if let Some(wid) = wid {
            client.option("wid", &wid.to_string())?;
            client.option("vo", "gpu-next")?;
            client.option("force-window", "yes")?;
        } else {
            // CLI smoke tests exercise demux/decode without an AppKit event loop.
            client.option("vo", "null")?;
            client.option("ao", "null")?;
        }
        client.check(unsafe { (client.api.initialize)(handle) })?;
        Ok(client)
    }

    fn check(&self, result: c_int) -> AppResult<()> {
        if result >= 0 {
            return Ok(());
        }
        let message = unsafe { CStr::from_ptr((self.api.error)(result)) }.to_string_lossy();
        Err(AppError::new("PLAYER", message.into_owned()))
    }
    fn option(&self, name: &str, value: &str) -> AppResult<()> {
        let (name, value) = (cstring(name)?, cstring(value)?);
        self.check(unsafe { (self.api.option)(self.handle, name.as_ptr(), value.as_ptr()) })
    }
    pub fn command(&mut self, args: &[&str]) -> AppResult<()> {
        let args = args
            .iter()
            .map(|arg| cstring(arg))
            .collect::<AppResult<Vec<_>>>()?;
        let mut pointers: Vec<_> = args.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(std::ptr::null());
        self.check(unsafe { (self.api.command)(self.handle, pointers.as_ptr()) })
    }
    pub fn set(&mut self, name: &str, value: impl ToString) -> AppResult<()> {
        self.command(&["set", name, &value.to_string()])
    }
    pub fn screenshot_rgb(&mut self) -> AppResult<(usize, usize, Vec<u8>)> {
        let args = [
            cstring("screenshot-raw")?,
            cstring("video")?,
            cstring("rgba")?,
        ];
        let pointers = [
            args[0].as_ptr(),
            args[1].as_ptr(),
            args[2].as_ptr(),
            std::ptr::null(),
        ];
        let mut node = Node {
            data: NodeData { integer: 0 },
            format: 0,
        };
        self.check(unsafe { (self.api.command_ret)(self.handle, pointers.as_ptr(), &mut node) })?;
        // The borrowed image is copied before freeing its result node.
        let result = unsafe { copy_screenshot(&node) };
        unsafe { (self.api.free_node)(&mut node) };
        result
    }
    pub fn property(&self, name: &str) -> AppResult<Value> {
        let name = cstring(name)?;
        let mut node = Node {
            data: NodeData { integer: 0 },
            format: 0,
        };
        let result = unsafe {
            (self.api.get)(
                self.handle,
                name.as_ptr(),
                6,
                (&mut node as *mut Node).cast(),
            )
        };
        if result == -10 {
            return Ok(Value::Null);
        }
        self.check(result)?;
        // SAFETY: a successful get owns the returned tree until free_node.
        let value = unsafe { node_value(&node) };
        unsafe { (self.api.free_node)(&mut node) };
        Ok(value)
    }
    pub fn drain_events(&mut self) {
        // The event pointer is valid only until the next wait_event.
        loop {
            let event = unsafe { &*(self.api.event)(self.handle, 0.0) };
            match event.id {
                0 => break,
                6 => {
                    self.loaded = false;
                    self.last_error = None;
                }
                8 => self.loaded = true,
                21 => self.frame_revision = self.frame_revision.wrapping_add(1),
                7 if !event.data.is_null() => {
                    let end = unsafe { &*event.data.cast::<EndFile>() };
                    if end.reason == 4 {
                        self.last_error = Some(
                            unsafe { CStr::from_ptr((self.api.error)(end.error)) }
                                .to_string_lossy()
                                .into_owned(),
                        );
                    }
                    if end.reason != 0 {
                        self.loaded = false;
                    }
                }
                _ => {}
            }
        }
    }
}

unsafe fn copy_screenshot(node: &Node) -> AppResult<(usize, usize, Vec<u8>)> {
    let invalid = || AppError::new("FRAME_DATA", "解码器返回的画面格式无效。");
    if node.format != 8 || node.data.list.is_null() {
        return Err(invalid());
    }
    let list = &*node.data.list;
    if !(1..=32).contains(&list.count) || list.values.is_null() || list.keys.is_null() {
        return Err(invalid());
    }
    let values = std::slice::from_raw_parts(list.values, list.count as usize);
    let keys = std::slice::from_raw_parts(list.keys, list.count as usize);
    let get = |name: &str| {
        keys.iter()
            .zip(values)
            .find(|(key, _)| !key.is_null() && CStr::from_ptr(**key).to_bytes() == name.as_bytes())
            .map(|(_, v)| v)
    };
    let integer = |key| {
        get(key)
            .filter(|v| v.format == 4)
            .map(|v| v.data.integer)
            .unwrap_or(0)
    };
    let (width, height, stride) = (integer("w"), integer("h"), integer("stride"));
    if !(1..=4096).contains(&width)
        || !(1..=4096).contains(&height)
        || stride < width * 4
        || stride > 65536
    {
        return Err(invalid());
    }
    let data = get("data")
        .filter(|v| v.format == 9 && !v.data.bytes.is_null())
        .ok_or_else(invalid)?;
    let bytes = &*data.data.bytes;
    let required = ((height - 1) * stride + width * 4) as usize;
    if bytes.data.is_null() || bytes.size < required {
        return Err(invalid());
    }
    let bytes = std::slice::from_raw_parts(bytes.data.cast::<u8>(), required);
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let i = y * stride as usize + x * 4;
            rgb.extend_from_slice(&bytes[i..i + 3]);
        }
    }
    Ok((width as usize, height as usize, rgb))
}
impl Drop for Client {
    fn drop(&mut self) {
        unsafe { (self.api.destroy)(self.handle) };
    }
}

// Inspect only the union member selected by format; ignore unknown formats.
unsafe fn node_value(node: &Node) -> Value {
    match node.format {
        1 if !node.data.string.is_null() => Value::String(
            CStr::from_ptr(node.data.string)
                .to_string_lossy()
                .into_owned(),
        ),
        3 => Value::Bool(node.data.flag != 0),
        4 => Value::from(node.data.integer),
        5 => Value::from(node.data.double),
        7 | 8 if !node.data.list.is_null() => {
            let list = &*node.data.list;
            if list.count <= 0 {
                return if node.format == 7 {
                    Value::Array(vec![])
                } else {
                    serde_json::json!({})
                };
            }
            let values = std::slice::from_raw_parts(list.values, list.count as usize);
            if node.format == 7 {
                Value::Array(values.iter().map(|v| node_value(v)).collect())
            } else {
                let keys = std::slice::from_raw_parts(list.keys, list.count as usize);
                Value::Object(
                    keys.iter()
                        .zip(values)
                        .map(|(k, v)| {
                            (
                                CStr::from_ptr(*k).to_string_lossy().into_owned(),
                                node_value(v),
                            )
                        })
                        .collect(),
                )
            }
        }
        _ => Value::Null,
    }
}
