use std::{mem, ptr};
use std::ptr::NonNull;

use winapi::um::errhandlingapi::*;
use winapi::um::handleapi::*;
use winapi::um::memoryapi::*;
use winapi::um::winnt::*;
use winapi::um::sysinfoapi::*;

#[inline]
pub fn granularity() -> usize {
	let mut si = mem::MaybeUninit::<SYSTEM_INFO>::uninit();
	let si = unsafe {
		GetSystemInfo(si.as_mut_ptr());
		si.assume_init()
	};
	si.dwAllocationGranularity as usize
}

#[inline(never)]
pub unsafe fn allocate(cap: usize, size_of: usize) -> (NonNull<u8>, usize) {
	if cap == 0 {
		return (NonNull::dangling(), 0);
	}

	// Round capacity to nearest multiple of the system's allocation granularity
	let cap = super::round_capacity(cap, size_of);

	let cap_high = (cap as u64 >> 32) as u32;
	let cap_low = (cap as u64 & 0xffffffff) as u32;

	let map = CreateFileMappingW(ptr::null_mut(), ptr::null_mut(), PAGE_READWRITE, cap_high, cap_low, ptr::null());
	if map.is_null() {
		error("CreateFileMapping")
	}

	// Race condition between replacing the virtual memory with file mapping
	// Attempt it a couple of times and give up otherwise
	for _ in 0..10 {
		let base = VirtualAlloc(ptr::null_mut(), cap + cap, MEM_RESERVE, PAGE_READWRITE);
		if base.is_null() {
			CloseHandle(map);
			error("VirtualAlloc")
		}
		VirtualFree(base, 0, MEM_RELEASE);

		let p1 = MapViewOfFileEx(map, FILE_MAP_READ|FILE_MAP_WRITE, 0, 0, cap, base);
		let p2 = MapViewOfFileEx(map, FILE_MAP_READ|FILE_MAP_WRITE, 0, 0, cap, (base as usize + cap) as *mut _);

		if !p1.is_null() && !p2.is_null() {
			// FIXME! I'm pretty sure it's not okay to close the file mapping handle while using the mapped views
			CloseHandle(map);
			return (NonNull::new_unchecked(base as *mut u8), cap);
		}

		if !p1.is_null() {
			UnmapViewOfFile(p1);
		}
		if !p2.is_null() {
			UnmapViewOfFile(p2);
		}
	}

	CloseHandle(map);
	error("MapViewOfFileEx")
}

#[inline]
pub unsafe fn free(ptr: NonNull<u8>, cap: usize) {
	let ptr = ptr.as_ptr();
	UnmapViewOfFile(ptr as _);
	UnmapViewOfFile(ptr.add(cap) as _);
}

#[cold]
#[track_caller]
fn error(name: &str) -> ! {
	panic!("{}(): {}", name, unsafe { GetLastError() })
}
