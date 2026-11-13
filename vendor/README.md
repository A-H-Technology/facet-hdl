# vendor/facet-core

facet-core 0.50.0-rc.7 from crates.io, with one change in
`src/types/ptr/tagged.rs` (marked "facet-hdl's patch"). Upstream tags wide
pointers with the address's top bit, assuming user space never uses it. On
32-bit ARM Linux it does: the stack sits at 0xBExx_xxxx. So `bind` panicked
on the Zybo ("as_byte_ptr called on wide pointer"). qemu-user maps the
guest low, so it never showed there.

Upstream: not filed yet. Remove this directory and the `[patch.crates-io]`
entry in Cargo.toml once a facet release keeps the flag out of the address
on 32-bit targets.
