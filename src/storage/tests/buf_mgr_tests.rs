use std::fs::{File, remove_file};
use std::io::{Write};
use storage::PAGE_SIZE;
use storage::buf_key::BufKey;
use storage::buf_mgr::BufMgr;

#[test]
fn test_bufmgr_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<BufMgr>();
    assert_sync::<BufMgr>();
}

#[test]
fn test_bufmgr_get() {
    let data_file = "1.dat";

    setup_bufmgr(data_file);
    let mut buf_mgr = BufMgr::new(None);
    let buf_page = buf_mgr.get_buf(&BufKey::new(1, 0)).unwrap();
    teardown_bufmgr(data_file);

    let lock = buf_page.read().unwrap();
    assert_eq!(lock.buf.len(), PAGE_SIZE);
    for byte in lock.buf.iter() { assert_eq!(*byte, 0); }
}

#[test]
fn test_bufmgr_store() {
    let data_file = "2.dat";

    setup_bufmgr(data_file);
    let mut buf_mgr = BufMgr::new(None);
    {
        let buf_page = buf_mgr.get_buf(&BufKey::new(2, 0)).unwrap();
        // Change values in buf_page
        let mut lock = buf_page.write().unwrap();
        lock.buf[0] = 1;
        lock.buf[1] = 1;
    }
    // Write buf page
    buf_mgr.store_buf(&BufKey::new(2, 0)).unwrap();

    let mut buf_mgr = BufMgr::new(None);
    let buf_page = buf_mgr.get_buf(&BufKey::new(2, 0)).unwrap();
    teardown_bufmgr(data_file);

    let lock = buf_page.read().unwrap();
    assert_eq!(lock.buf[0], 1);
    assert_eq!(lock.buf[1], 1);
}

#[test]
fn test_bufmgr_new_buf() {
    let data_file = "3.dat";

    setup_bufmgr(data_file);
    let mut buf_mgr = BufMgr::new(None);
    assert!(buf_mgr.new_buf(&BufKey::new(3, 2)).is_err());
    assert!(buf_mgr.new_buf(&BufKey::new(3, 0)).is_err());

    let _buf_page = buf_mgr.new_buf(&BufKey::new(3, 1)).unwrap();
    teardown_bufmgr(data_file);
}

#[test]
fn test_bufmgr_evict() {
    let data_file = "4.dat";

    setup_bufmgr(data_file);
    // BufMgr of max 3 pages
    let mut buf_mgr = BufMgr::new(Some(3));
    buf_mgr.new_buf(&BufKey::new(4, 1)).unwrap();
    buf_mgr.new_buf(&BufKey::new(4, 2)).unwrap();
    buf_mgr.new_buf(&BufKey::new(4, 3)).unwrap();

    // Queue: page-1  page-2  page-3
    // Ref:     1       1       1
    // Hand:    ^
    buf_mgr.get_buf(&BufKey::new(4, 0)).unwrap();
    assert!(!buf_mgr.has_buf(&BufKey::new(4, 1)));

    // Queue: page-2  page-3  page-0
    // Ref:     0       0       1
    // Hand:    ^
    buf_mgr.get_buf(&BufKey::new(4, 1)).unwrap();
    assert!(!buf_mgr.has_buf(&BufKey::new(4, 2)));

    // Queue: page-3  page-0  page-1
    // Ref:     0       1       1
    // Hand:    ^
    buf_mgr.get_buf(&BufKey::new(4, 3)).unwrap();

    // Queue: page-3  page-0  page-1
    // Ref:     1       1       1
    // Hand:    ^
    buf_mgr.get_buf(&BufKey::new(4, 2)).unwrap();
    assert!(!buf_mgr.has_buf(&BufKey::new(4, 3)));

    // Queue: page-0  page-1  page-2
    // Ref:     0       0       1
    // Hand:    ^
    buf_mgr.get_buf(&BufKey::new(4, 0)).unwrap();
    buf_mgr.get_buf(&BufKey::new(4, 3)).unwrap();
    assert!(!buf_mgr.has_buf(&BufKey::new(4, 1)));

    // Queue: page-0  page-2  page-3
    // Ref:     0       1       1
    // Hand:            ^
    let _buf_two = buf_mgr.get_buf(&BufKey::new(4, 2)).unwrap();
    buf_mgr.get_buf(&BufKey::new(4, 0)).unwrap();
    buf_mgr.get_buf(&BufKey::new(4, 1)).unwrap();
    assert!(!buf_mgr.has_buf(&BufKey::new(4, 3)));
    teardown_bufmgr(data_file);
}

fn setup_bufmgr(data_file: &str) {
    let mut file = File::create(data_file).unwrap();
    file.write_all(&[0; PAGE_SIZE as usize]).unwrap();
}

fn teardown_bufmgr(data_file: &str) {
    remove_file(data_file).unwrap();
}
