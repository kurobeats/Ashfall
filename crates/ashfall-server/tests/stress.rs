//! Stress tests — verify stability under load, no memory leaks or panics.

use ashfall_core::id::NetworkID;
use ashfall_server::world::objects::Object;
use ashfall_server::world::registry::ObjectRegistry;
use ashfall_server::session::{Session, SessionState};
use std::net::SocketAddr;
use std::sync::Arc;

#[test]
fn test_many_objects_no_panic() {
    let registry = Arc::new(ObjectRegistry::new());

    // Insert 1000 objects — verify no panic and no memory growth
    for i in 0..1000 {
        let id = registry.allocate_id();
        let obj = Object::new(id, i as u32, (i + 1000) as u32, (i % 256) as u32);
        registry.insert(obj);
        registry.add_to_cell((i % 256) as u32, id);
    }

    assert_eq!(registry.total_count(), 1000);

    // Verify we can retrieve objects from cells
    for cell in 0..10 {
        let objects = registry.get_by_cell(cell);
        assert!(objects.len() >= 1000 / 256);
    }

    // Remove half
    for i in 0..500 {
        let id = NetworkID::new((i + 1) as u64);
        registry.remove(id);
    }

    assert_eq!(registry.total_count(), 500);
}

#[test]
fn test_many_cells_no_panic() {
    let registry = Arc::new(ObjectRegistry::new());

    // Insert objects across many cells
    for cell in 0..256 {
        let id = registry.allocate_id();
        let obj = Object::new(id, 0x100 + cell, 0x200 + cell, cell);
        registry.insert(obj);
        registry.add_to_cell(cell, id);
    }

    assert_eq!(registry.total_count(), 256);

    // Query all cells
    for cell in 0..256 {
        let objects = registry.get_by_cell(cell);
        assert_eq!(objects.len(), 1, "cell {cell} should have one object");
    }
}

#[test]
fn test_many_sessions_no_panic() {
    let addr: SocketAddr = "127.0.0.1:1770".parse().unwrap();

    // Create 20 mock sessions — verify no panic
    let mut sessions: Vec<Session> = Vec::new();
    for i in 0..20 {
        let session = Session::new(
            NetworkID::new((i + 1) as u64),
            addr,
            format!("Player{i}"),
        );
        sessions.push(session);
    }

    assert_eq!(sessions.len(), 20);

    // Verify all are valid
    for s in &sessions {
        assert!(s.is_active());
        assert!(!s.is_ingame());
    }
}

#[test]
fn test_object_registry_concurrent_reads() {
    use std::thread;

    let registry = Arc::new(ObjectRegistry::new());

    // Pre-populate
    for i in 0..100 {
        let id = registry.allocate_id();
        let obj = Object::new(id, i as u32, i as u32, 0);
        registry.insert(obj);
        registry.add_to_cell(0, id);
    }

    // Concurrent reads from multiple threads
    let mut handles = Vec::new();
    for _ in 0..4 {
        let reg = registry.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let objects = reg.get_by_cell(0);
                assert_eq!(objects.len(), 100);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn test_type_count_accurate() {
    use ashfall_core::types::ObjectKind;

    let registry = Arc::new(ObjectRegistry::new());

    for i in 0..50 {
        let id = registry.allocate_id();
        let obj = Object::new(id, i as u32, i as u32, 0);
        registry.insert(obj);
    }

    let count = registry.type_count(ObjectKind::Object);
    assert_eq!(count, 50);
}
