RingBuffer
==========

[![MIT License](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![crates.io](https://img.shields.io/crates/v/vringbuf.svg)](https://crates.io/crates/vringbuf)
[![docs.rs](https://docs.rs/vringbuf/badge.svg)](https://docs.rs/vringbuf)
[![Build status](https://github.com/CasualX/ringbuf/workflows/CI/badge.svg)](https://github.com/CasualX/ringbuf/actions)

Examples
--------

```rust
use vringbuf::RingBuffer;

static DATA: [u8; 400] = [0xef; 400];

let mut rbuf = RingBuffer::<u8>::new();

// Repeatedly append data and remove it from the tail
for _ in 0..1000 {
	rbuf.extend_from_slice(&DATA);
	rbuf.remove_tail(DATA.len());
}
```

License
-------

Licensed under [MIT License](https://opensource.org/licenses/MIT), see [license.txt](license.txt).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.
