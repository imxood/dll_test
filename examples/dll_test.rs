use std::time::Duration;

use libloading::{Library, Symbol};

fn load_dll() {
    let mut i = 10;
    while i > 0 {
        i -= 1;

        let lib = unsafe {
            Library::new(r"target\release\dll_test.dll").unwrap()
        };

        unsafe {
            let init_user: Symbol<extern "C" fn()> = lib.get(b"init_user").unwrap();
            init_user();
        }

        println!("1");
        lib.close().unwrap();
    }
}

fn main() {
    load_dll();
}
