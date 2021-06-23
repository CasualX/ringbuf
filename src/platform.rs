// Implement mirrored memory for the right platform
//
// Each platform specific module must export:
// * pub fn granularity() -> usize;
// * pub unsafe fn allocate(cap: usize) -> (NonNull<u8>, usize);
// * pub unsafe fn free(ptr: *mut u8, cap: usize);

cfg_if::cfg_if! {
	if #[cfg(windows)] {
		mod windows;
		pub use self::windows::*;
	}
	else if #[cfg(any(target_os = "macos", target_os = "ios"))] {
		mod macos;
		pub use self::macos::*;
	}
	else if #[cfg(unix)] {
		mod linux;
		pub use self::linux::*;
	}
	else {
		compile_error!("unsupported platform!")
	}
}

fn round_capacity(cap: usize, size_of: usize) -> usize {
	let g = granularity();
	let cap = match cap.checked_mul(size_of) {
		Some(cap) => cap,
		None => invalid_capacity(cap),
	};
	let cap = ((cap - 1) & !(g - 1)) + g;
	if cap == 0 || cap >= isize::MAX as usize / 2 {
		invalid_capacity(cap);
	}
	cap
}

#[cold]
#[track_caller]
pub fn invalid_capacity(cap: usize) -> ! {
	panic!("invalid capacity: {:#x}", cap)
}
