//! World sync integration tests — cell context, object lifecycle.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::{GameObject, ObjectKind};
use ashfall_server::world::cell::CellContext;
use ashfall_server::world::objects::Object;
use ashfall_server::world::registry::ObjectRegistry;
use std::sync::Arc;

#[test]
fn test_cell_context_enter_leave() {
    // Cell 0x01000002 → neighbors include 0x01000102, 0x01000001, etc.
    let old = CellContext::new(0x01000002);
    // Move to cell 0x01000004
    let new = CellContext::new(0x01000004);

    let (enter, leave) = old.diff(&new);
    assert!(!enter.is_empty(), "should have enter cells");
    assert!(!leave.is_empty(), "should have leave cells");
    // Old center cell (0x01000002) should be in leave
    assert!(leave.contains(&0x01000002), "old center should be in leave cells");
    // New center cell (0x01000004) should be in enter
    assert!(enter.contains(&0x01000004), "new center should be in enter cells");
}

#[test]
fn test_cell_context_same_cell_no_diff() {
    let ctx = CellContext::new(0x01000002);
    let (enter, leave) = ctx.diff(&ctx);
    assert!(enter.is_empty(), "same context should have no enter cells");
    assert!(leave.is_empty(), "same context should have no leave cells");
}

#[test]
fn test_object_create_and_move() {
    let registry = Arc::new(ObjectRegistry::new());
    let id = registry.allocate_id();

    // Create object
    let obj = Object::new(id, 0x100, 0x200, 5);
    obj.to_new_packet(); // verify serializable
    registry.insert(obj);
    registry.add_to_cell(5, id);

    // Verify in registry
    let retrieved = registry.get(id);
    assert!(retrieved.is_some(), "object should be in registry");

    // Verify in cell
    let cell_objects = registry.get_by_cell(5);
    assert!(cell_objects.contains(&id), "object should be in cell 5");

    // Move object — update position
    {
        let arc = registry.get(id).unwrap();
        let mut guard = arc.write();
        if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
            obj.net_pos = [100.0, 200.0, 0.0];
        }
    }

    // Verify position updated
    {
        let arc = registry.get(id).unwrap();
        let guard = arc.read();
        if let Some(obj) = guard.as_any().downcast_ref::<Object>() {
            assert_eq!(obj.net_pos, [100.0, 200.0, 0.0]);
        } else {
            panic!("not an Object");
        }
    }

    // Remove object
    registry.remove(id);
    assert!(registry.is_deleted(id), "object should be marked deleted");
}

#[test]
fn test_object_new_packet_produces_serializable() {
    let id = NetworkID::new(42);
    let obj = Object::new(id, 0x100, 0x200, 7);
    let packet = obj.to_new_packet();

    let bytes = postcard::to_stdvec(&packet).expect("serialize");
    let _: Packet = postcard::from_bytes(&bytes).expect("deserialize");
}
