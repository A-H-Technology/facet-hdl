//! The whole contract between the HPS and the fabric: one call each way.
//! `hdl/generated/` comes from this file, and so do the host's handles.

#[facet_hdl::rpc]
pub mod hello {
    /// Runs in the fabric (`hdl/hello_fabric.spade`); Rust calls it.
    #[import]
    pub fn add_one(x: u32) -> u32;

    /// Runs here; the fabric calls it each time the button goes down.
    #[export(oneway)]
    pub fn pressed() {
        println!("hello from the fabric!");
    }
}
