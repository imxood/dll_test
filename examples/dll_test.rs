use std::{
    ffi::{c_char, c_void, CString},
    time::{Duration, Instant},
};

use dll_test::Profile;
use libloading::{Library, Symbol};
use tokio::sync::mpsc::UnboundedSender;

fn load_dll() {
    let mut i = 10;
    while i > 0 {
        i -= 1;

        let lib = unsafe { Library::new(r"target\debug\dll_test.dll").unwrap() };

        unsafe {
            // 下面的大括号只是为了直观

            // 测试 println
            {
                let test_println: Symbol<extern "C" fn()> = lib.get(b"test_println").unwrap();
                test_println();
            }

            // 测试 Box::into_raw()
            {
                let init_profile: Symbol<extern "C" fn() -> *mut c_void> =
                    lib.get(b"init_profile").unwrap();

                // // 泄露堆内存
                // let _ = init_profile();

                // 自动释放堆内存
                let profile = init_profile();
                let _b = Box::from_raw(profile as *mut Profile);
            }

            // // 测试 OncLock, 它导致了内存泄露, 而且 没不到释放的办法
            // {
            //     let test_oncelock = lib.get::<extern "C" fn()>(b"test_oncelock").unwrap();
            //     test_oncelock();
            // }

            // 测试 static mut, 它没那么安全, 但是不会导致内存泄露
            {
                let static_mut_init = lib.get::<extern "C" fn()>(b"static_mut_init").unwrap();
                static_mut_init();

                let static_mut_deinit = lib.get::<extern "C" fn()>(b"static_mut_deinit").unwrap();
                static_mut_deinit();
            }

            // 测试 log crate, 卸载库时 是否 panic
            {
                let my_log_init: Symbol<extern "C" fn()> = lib.get(b"my_log_init").unwrap();
                my_log_init();

                let my_log_info: Symbol<extern "C" fn(*const c_char)> =
                    lib.get(b"my_log_info").unwrap();

                let msg = CString::new("hello").unwrap();
                my_log_info(msg.as_ptr());

                // 这里不会打印日志, 主进程中 没有设置 logger, 前面设置的 属于动态库的logger
                log::info!("hello1");
            }

            /* tokio 运行时, 使用了 OnceLock, 会导致内存泄露 */

            // {
            //     let tokio_init: Symbol<extern "C" fn() -> *mut c_void> =
            //         lib.get(b"tokio_init").unwrap();
            //     let tx = tokio_init();

            //     let tokio_send: Symbol<extern "C" fn(*const c_void)> =
            //         lib.get(b"tokio_send").unwrap();
            //     tokio_send(tx);

            //     // 由堆上的指针 转给 rust的Box, 被rust管理, 即 离开作用域就自动释放
            //     let mut tx = Box::from_raw(tx as *mut UnboundedSender<u8>);

            //     let now = Instant::now();

            //     // 卸载
            //     let tokio_deinit: Symbol<extern "C" fn()> = lib.get(b"tokio_deinit").unwrap();
            //     tokio_deinit();

            //     println!("elapsed: {:?}", now.elapsed());

            //     println!("tokio_deinit done");
            // }

            /* tokio 运行时, 使用了 static mut, 虽然不会内存泄露, 但是 需要手动释放 */

            {
                let now = Instant::now();

                let tokio_init0: Symbol<extern "C" fn()> = lib.get(b"tokio_init0").unwrap();
                tokio_init0();

                let tokio_deinit0: Symbol<extern "C" fn()> = lib.get(b"tokio_deinit0").unwrap();
                tokio_deinit0();

                println!("elapsed: {:?}", now.elapsed());

                println!("tokio_deinit done");
            }
        }

        println!("{i}");
        lib.close().unwrap();
    }
}

fn main() {
    load_dll();
}
