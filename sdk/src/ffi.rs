use crate::engine::BigDataEngine;
use std::ffi::{CStr, CString};
use std::io::Read;
use std::os::raw::c_char;
use std::path::Path;

/// Opaque pointer to the Zen Engine.
pub struct ZenEngineHandle(BigDataEngine);

#[no_mangle]
pub extern "C" fn zen_engine_new(path: *const c_char) -> *mut ZenEngineHandle {
    if path.is_null() { return std::ptr::null_mut(); }
    
    let c_str = unsafe { CStr::from_ptr(path) };
    let path_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    
    match BigDataEngine::new(Path::new(path_str), crate::types::HardwareMode::Auto) {
        Ok(engine) => Box::into_raw(Box::new(ZenEngineHandle(engine))),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn zen_engine_free(handle: *mut ZenEngineHandle) {
    if !handle.is_null() {
        unsafe { let _ = Box::from_raw(handle); }
    }
}

#[no_mangle]
pub extern "C" fn zen_engine_get_row_count(handle: *mut ZenEngineHandle) -> u64 {
    if handle.is_null() { return 0; }
    let engine = unsafe { &(*handle).0 };
    if engine.is_compressed {
        0 // Row count unknown until analyzed for compressed files (unless indexed)
    } else {
        engine.offsets.len() as u64
    }
}

#[no_mangle]
pub extern "C" fn zen_engine_search_json(
    handle: *mut ZenEngineHandle, 
    query: *const c_char,
    limit: usize
) -> *mut c_char {
    if handle.is_null() || query.is_null() { return std::ptr::null_mut(); }
    
    let engine = unsafe { &(*handle).0 };
    let q_str = unsafe { CStr::from_ptr(query).to_string_lossy() };
    
    // Use raw forensic scanner (zero-index) so external wrappers 
    // don't have to worry about index build lifecycle for basic searches
    match engine.search_raw(&q_str, limit, false, true, false) {
        Ok(results) => {
            let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
            CString::new(json).unwrap().into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn zen_engine_benchmark_search(
    handle: *mut ZenEngineHandle,
    query: *const c_char,
) -> *mut c_char {
    if handle.is_null() || query.is_null() { return std::ptr::null_mut(); }
    
    let engine = unsafe { &(*handle).0 };
    let q_str = unsafe { CStr::from_ptr(query).to_string_lossy() };
    let needle = q_str.as_bytes();
    let finder = memchr::memmem::Finder::new(needle);
    
    let data = &engine.mmap;
    let mut count = 0;
    let start_time = std::time::Instant::now();
    let file_size;

    if engine.is_compressed {
        // High-performance streaming benchmark for compressed files
        if let Ok(mut decompressor) = crate::compression::get_decompressor(&engine.path) {
            let mut buf = vec![0u8; 128 * 1024 * 1024];
            let mut total_decompressed = 0;
            while let Ok(n) = decompressor.read(&mut buf) {
                if n == 0 { break; }
                let chunk = &buf[..n];
                let mut chunk_offset = 0;
                while let Some(m) = finder.find(&chunk[chunk_offset..]) {
                    count += 1;
                    chunk_offset += m + needle.len();
                }
                total_decompressed += n;
            }
            file_size = total_decompressed;
        } else {
            return std::ptr::null_mut();
        }
    } else {
        // Scaneo completo real de todo el archivo (sin límite de parada)
        let mut offset = 0;
        let chunk_size = 128 * 1024 * 1024;
        
        while offset < data.len() {
            let end = (offset + chunk_size).min(data.len());
            let chunk = &data[offset..end];
            let mut chunk_offset = 0;
            while let Some(m) = finder.find(&chunk[chunk_offset..]) {
                count += 1;
                chunk_offset += m + needle.len();
            }
            offset += chunk_size;
        }
        file_size = data.len();
    }
    
    let duration = start_time.elapsed();
    let millis = duration.as_millis() as u64;
    let mb_ps = if millis > 0 {
        ((file_size as f64 / 1024.0 / 1024.0) / (millis as f64 / 1000.0)) as u64
    } else { 0 };

    let result = serde_json::json!({
        "count": count,
        "millis": millis,
        "throughput_mbs": mb_ps,
        "uncompressed_size_gb": (file_size as f64 / 1024.0 / 1024.0 / 1024.0)
    });

    let json = serde_json::to_string(&result).unwrap();
    CString::new(json).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn zen_engine_build_index(handle: *mut ZenEngineHandle) -> u64 {
    if handle.is_null() { return 0; }
    let engine = unsafe { &mut (*handle).0 };
    
    let start = std::time::Instant::now();
    // Usamos modo Auto para detectar si es SSD/UFS o HDD
    match engine.build_index() {
        Ok(_) => start.elapsed().as_millis() as u64,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "C" fn zen_engine_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { let _ = CString::from_raw(s); }
    }
}
