use std::ptr;
use std::ptr::NonNull;

#[inline]
pub fn granularity() -> usize {
	unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

#[inline(never)]
pub unsafe fn allocate(cap: usize, size_of: usize) -> (NonNull<u8>, usize) {
	if cap == 0 {
		return (NonNull::dangling(), 0);
	}

	// Round capacity to nearest multiple of the system's allocation granularity
	let cap = super::round_capacity(cap, size_of);

	// Create the file backing the ring buffer
	let fd = libc::syscall(libc::SYS_memfd_create, b"ringbuf\0".as_ptr(), 0) as i32;
	if fd < 0 {
		error("memfd_create")
	}
	if libc::ftruncate(fd, cap as libc::off_t) != 0 {
		libc::close(fd);
		error("ftruncate")
	}

	// Reserve memory for twice the capacity
	let base = libc::mmap(ptr::null_mut(), cap + cap, libc::PROT_NONE, libc::MAP_PRIVATE|libc::MAP_ANONYMOUS, -1, 0);
	if base == libc::MAP_FAILED || base.is_null() {
		libc::close(fd);
		error("mmap")
	}

	// Replace the reserved memory with the ring buffer mapping
	let addr1 = base;
	let ptr1 = libc::mmap(addr1, cap, libc::PROT_READ|libc::PROT_WRITE, libc::MAP_SHARED|libc::MAP_FIXED, fd, 0);
	let addr2 = (base as *mut u8).add(cap) as *mut libc::c_void;
	let ptr2 = libc::mmap(addr2, cap, libc::PROT_READ|libc::PROT_WRITE, libc::MAP_SHARED|libc::MAP_FIXED, fd, 0);

	libc::close(fd);

	if addr1 == ptr1 && addr2 == ptr2 {
		return (NonNull::new_unchecked(base as *mut u8), cap);
	}

	libc::munmap(base, cap + cap);
	error("mmap")
}

#[inline]
pub unsafe fn free(ptr: NonNull<u8>, cap: usize) {
	let ptr = ptr.as_ptr();
	libc::munmap(ptr as *mut libc::c_void, cap + cap);
}

#[cold]
#[track_caller]
fn error(name: &str) -> ! {
	let errno = unsafe { *libc::__errno_location() };
	panic!("{}(): {}", name, errno)
}
