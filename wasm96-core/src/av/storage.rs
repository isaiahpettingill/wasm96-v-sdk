// Needed for `alloc::` in this crate.
extern crate alloc;

use crate::state::global;
use wasmtime::Caller;

// External crates for rendering

// External crates for asset decoding

// Storage ABI helpers
use super::utils::{guest_alloc, guest_free};

pub fn storage_save(env: &mut Caller<'_, ()>, key: u64, data_ptr: u32, data_len: u32) {
    // Read guest memory pointers
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };
    if memory_ptr.is_null() {
        return;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    let mut data = vec![0u8; data_len as usize];
    if mem.read(&mut *env, data_ptr as usize, &mut data).is_err() {
        return;
    }

    let mut s = global().lock().unwrap();
    s.storage.kv.insert(key, data);
}

pub fn storage_load(env: &mut Caller<'_, ()>, key: u64) -> u64 {
    // Read guest memory pointers
    let memory_ptr = {
        let s = global().lock().unwrap();
        s.memory_wasmtime
    };
    if memory_ptr.is_null() {
        return 0;
    }

    // SAFETY: memory pointer checked.
    let mem = unsafe { &*memory_ptr };

    let data = {
        let s = global().lock().unwrap();
        match s.storage.kv.get(&key) {
            Some(v) => v.clone(),
            None => return 0,
        }
    };

    let Some(dst_ptr) = guest_alloc(env, data.len() as u32) else {
        return 0;
    };

    // Write to guest memory
    if mem.write(&mut *env, dst_ptr as usize, &data).is_err() {
        // If write fails, attempt to free.
        guest_free(env, dst_ptr, data.len() as u32);
        return 0;
    }

    ((dst_ptr as u64) << 32) | (data.len() as u64)
}

pub fn storage_free(env: &mut Caller<'_, ()>, ptr: u32, len: u32) {
    guest_free(env, ptr, len);
}
