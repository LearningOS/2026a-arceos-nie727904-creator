use std::fs::File;
use std::io::{self, Read};
use alloc::vec::Vec;

use axhal::mem::PAGE_SIZE_4K;
use axhal::paging::MappingFlags;
use axmm::AddrSpace;

use crate::VM_ENTRY;

pub fn load_vm_image(fname: &str, uspace: &mut AddrSpace) -> io::Result<()> {
    ax_println!("app: {}", fname);

    let mut file = File::open(fname)?;
    let mut image = Vec::new();
    let mut buf = [0u8; PAGE_SIZE_4K];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        image.extend_from_slice(&buf[..n]);
    }

    assert!(!image.is_empty(), "Guest image is empty");

    let map_size = image
        .len()
        .checked_add(PAGE_SIZE_4K - 1)
        .expect("Guest image too large")
        & !(PAGE_SIZE_4K - 1);

    ax_println!(
        "Guest image: {} bytes, mapping {} bytes",
        image.len(),
        map_size
    );

    uspace
        .map_alloc(
            VM_ENTRY.into(),
            map_size,
            MappingFlags::READ
                | MappingFlags::WRITE
                | MappingFlags::EXECUTE
                | MappingFlags::USER,
            true,
        )
        .expect("Failed to map guest image");

    uspace
        .write(VM_ENTRY.into(), &image)
        .expect("Failed to copy guest image");

    Ok(())
}
