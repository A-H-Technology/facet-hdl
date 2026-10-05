//! Uio's register path against a faked sysfs and a plain file standing in
//! for /dev/uioN (mmap works the same on both). The interrupt half needs a
//! real uio_pdrv_genirq device and isn't covered here.

use facet::Facet;
use facet_hdl::{FromFabric, ToFabric, Transport};
use facet_hdl_linux::Uio;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Facet)]
pub struct Regs {
    pub ctl: ToFabric<u64>,
    pub status: FromFabric<u32>,
}

/// `/sys/class/uio` with uio0 (someone else's) and uio1 (ours), and a
/// 4 KiB file as /dev/uio1.
fn fake(map_size: u64) -> (PathBuf, PathBuf) {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "facet-hdl-uio-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    let (sysfs, dev) = (root.join("sys"), root.join("dev"));
    for (uio, name, addr) in [("uio0", "other", 0x1000_0000u64), ("uio1", "facet-test", 0x2001_0000)] {
        let map0 = sysfs.join(uio).join("maps/map0");
        fs::create_dir_all(&map0).unwrap();
        fs::write(sysfs.join(uio).join("name"), format!("{name}\n")).unwrap();
        fs::write(map0.join("addr"), format!("{addr:#018x}\n")).unwrap();
        fs::write(map0.join("size"), format!("{map_size:#x}\n")).unwrap();
    }
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("uio1"), vec![0u8; 4096]).unwrap();
    (sysfs, dev)
}

#[test]
fn finds_the_device_by_name_and_maps_its_window() {
    let (sysfs, dev) = fake(0x1000);
    let mut uio = Uio::open_in(&sysfs, &dev, "facet-test", 4).unwrap();
    uio.write(3, 0xC0FF_EE00).unwrap();
    assert_eq!(uio.read(3).unwrap(), 0xC0FF_EE00);
    let on_disk = fs::read(dev.join("uio1")).unwrap();
    assert_eq!(
        &on_disk[12..16],
        &0xC0FF_EE00u32.to_ne_bytes(),
        "the write went through the mapping"
    );
    assert!(uio.read(4).is_err(), "outside the 4-word window");
    assert_eq!(
        uio.lock_path(),
        PathBuf::from("/run/lock/facet-hdl-window-0x20010000.lock"),
        "same lock DevMem would take for this window"
    );
}

#[test]
fn refuses_a_node_whose_reg_is_smaller_than_the_boundary() {
    let (sysfs, dev) = fake(0x8);
    let err = Uio::open_in(&sysfs, &dev, "facet-test", 4).err().unwrap();
    assert!(err.to_string().contains("fix the node's reg"), "{err}");
}

#[test]
fn names_the_device_it_could_not_find() {
    let (sysfs, dev) = fake(0x1000);
    let err = Uio::open_in(&sysfs, &dev, "nope", 4).err().unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn sizes_the_mapping_from_the_declaration() {
    let (sysfs, dev) = fake(0x1000);
    let words = facet_hdl::Boundary::of::<Regs>().unwrap().words();
    assert_eq!(words, 4, "fingerprint + 2 words of u64 + 1 of u32");
    Uio::open_in(&sysfs, &dev, "facet-test", words).unwrap();
}
