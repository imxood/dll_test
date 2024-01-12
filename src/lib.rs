use std::{
    ffi::{c_char, c_void},
    fs::OpenOptions,
    io::Write,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use tokio::{
    runtime::Runtime,
    sync::mpsc::{unbounded_channel, UnboundedSender},
};

pub struct Profile {
    name: String,
    age: u8,
    email: String,
    address: String,
    data: [u8; 1024],
}

impl Drop for Profile {
    fn drop(&mut self) {
        // println!("Profile dropped");
    }
}

impl Profile {
    pub fn new() -> Self {
        Profile {
            name: format!("xiaoming"),
            age: 0,
            email: format!("xiaoming@163.com"),
            address: format!("beijing"),
            data: [0; 1024],
        }
    }
}

/// 尝试分配 堆内存 并泄露出去
///
#[no_mangle]
pub extern "C" fn init_profile() -> *mut c_void {
    let profile = Box::new(Profile::new());
    Box::into_raw(profile) as *mut c_void
}

static PROFILE0: OnceLock<Profile> = OnceLock::new();

/// 测试 OnceLock, 这里存在内存泄露
///
#[no_mangle]
pub extern "C" fn test_oncelock() {
    PROFILE0.get_or_init(|| Profile::new());
}

/// 测试 静态可变 变量
///

static mut PROFILE1: Option<&'static Profile> = None;

#[no_mangle]
pub extern "C" fn static_mut_init() {
    // 使用 Box::leak 泄露出 rust中的对象 到自由的栈上, 不受生命周期限制
    // 但是在必要的时候, 手动释放内存
    unsafe { PROFILE1 = Some(Box::leak(Box::new(Profile::new()))) };
}

#[no_mangle]
pub extern "C" fn static_mut_deinit() {
    // 取出栈数据, 转化成Box, 由 rust 释放内存
    unsafe {
        let profile = PROFILE1.take().unwrap() as *const Profile as *mut Profile;
        let _ = Box::from_raw(profile);
    };
}

/// 简单的 println!() 会导致 内存泄露 / 最后的分析: 内部的stdout 使用了 OnceLock, OnceLock初始化时导致内存泄漏
///
#[no_mangle]
pub extern "C" fn test_println() {
    // println!("2");

    // ::std::io::_print(format_args!("2\n"));
}

/*
    测试 log
*/

pub struct MyLogger {
    _no_use: u128,
}

impl MyLogger {
    pub const fn new() -> Self {
        Self { _no_use: u128::MAX }
    }
}

impl log::Log for MyLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let now = chrono::Local::now()
                .format("%Y-%m-%d_%H-%M-%S%.3f")
                .to_string();

            let module = record.module_path().unwrap_or("unknown");
            let line = record.line().unwrap_or(0);

            // 使用 OpenOptions 打开文件，如果不存在则创建
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .append(true) // 打开文件时追加数据
                .open("logs/temp.log")
                .unwrap();

            // 写入日志
            file.write_all(
                format!(
                    "[{}][{:5}][{}:{}]: {}\n",
                    now,
                    record.level(),
                    module,
                    line,
                    record.args()
                )
                .as_bytes(),
            )
            .unwrap();
        }
    }

    fn flush(&self) {}
}

/// Box::new产生的堆内存，在 log crate 中 被leak, 没有办法释放该内存, 导致内存泄露,
/// 但如果 这里的 MyLogger 没有成员, 工具就没有检查到内存泄露, 那此时的 Box 的内存在哪呢?
///
/// 这里使用 static 就不会有泄露风险
///

static MY_LOGGER: MyLogger = MyLogger::new();
#[no_mangle]
pub extern "C" fn my_log_init() {
    // log::set_boxed_logger(Box::new(MyLogger::new())).unwrap();
    log::set_logger(&MY_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
}

#[no_mangle]
pub extern "C" fn my_log_info(c_str: *const c_char) {
    let info = unsafe { std::ffi::CStr::from_ptr(c_str).to_str().unwrap() };
    log::info!("{info}");
}

/*
    创建一个 runtime, 同时也要保证 可以手动 被卸载  !!! 这里使用的是 OnceLock, 它导致了内存泄漏
*/

static RUNTIME: OnceLock<Mutex<Option<Runtime>>> = OnceLock::new();

/// tokio 的测试, 主要测试 tokio运行时的安全卸载
///
#[no_mangle]
pub extern "C" fn tokio_init() -> *const c_void {
    let runtime = RUNTIME.get_or_init(|| {
        Mutex::new(Some(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap(),
        ))
    });

    // 栈上的数据
    let (tx, mut rx) = unbounded_channel::<u8>();

    // 栈上数据 转到 堆上
    let tx = Box::into_raw(Box::new(tx));

    // 创建一个 等待接收的 tokio 任务
    runtime.lock().unwrap().as_mut().unwrap().spawn(async move {
        while let Some(v) = rx.recv().await {
            let _ = v + 1;
        }
    });

    tx as *const c_void
}

#[no_mangle]
pub extern "C" fn tokio_send(tx: *const c_void) {
    let tx = unsafe { &*(tx as *const UnboundedSender<u8>) };
    let _ = tx.send(101);
}

#[no_mangle]
pub extern "C" fn tokio_deinit() {
    let runtime = RUNTIME.get().unwrap().lock().unwrap().take().unwrap();
    // 虽然设置超时1000ms, 但实测不到1ms 就结束了
    runtime.shutdown_timeout(Duration::from_millis(1000));
}

/*
    创建一个 runtime, 同时也要保证 可以手动 被卸载  !!! 这里使用的是 static mut, 它不会导致内存泄露, 但是 unsafe
*/

static mut RUNTIME0: Option<&'static Runtime> = None;

/// tokio 的测试, 主要测试 tokio运行时的安全卸载
///
#[no_mangle]
pub extern "C" fn tokio_init0() {
    unsafe {
        RUNTIME0 = Some(Box::leak(Box::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap(),
        )));
    }
}

#[no_mangle]
pub extern "C" fn tokio_deinit0() {
    unsafe {
        let runtime = RUNTIME0.take().unwrap();
        let runtime = Box::from_raw(runtime as *const Runtime as *mut Runtime);
        runtime.shutdown_timeout(Duration::from_millis(1000));
    }
}
