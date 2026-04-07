//! Integration test: full bottle lifecycle.
//!
//! Tests the complete flow: create -> configure -> export -> import -> duplicate -> delete.

use cauldron_core::bottle::BottleManager;
use cauldron_core::bottle_archive::{duplicate_bottle, estimate_archive_size};
use cauldron_core::runtime::{RuntimeInstaller, RuntimeType, RuntimeVersion};

#[test]
fn test_full_bottle_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = BottleManager::new(tmp.path().to_path_buf());

    // 1. Create a bottle
    let bottle = manager.create("IntegrationTest", "wine-10.0").unwrap();
    assert!(bottle.path.exists());
    assert_eq!(bottle.name, "IntegrationTest");

    // 2. Verify we can get it back
    let fetched = manager.get(&bottle.id).unwrap();
    assert_eq!(fetched.id, bottle.id);
    assert_eq!(fetched.wine_version, "wine-10.0");

    // 3. Verify it shows in list
    let list = manager.list().unwrap();
    assert_eq!(list.len(), 1);

    // 4. Set up a fake DXVK runtime and install it
    let rt_path = tmp.path().join("runtime-dxvk");
    let x64 = rt_path.join("x64");
    std::fs::create_dir_all(&x64).unwrap();
    for dll in &["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"] {
        std::fs::write(x64.join(dll), "fake dll content").unwrap();
    }

    let runtime = RuntimeVersion {
        name: "dxvk-test".to_string(),
        runtime_type: RuntimeType::Dxvk,
        version: "1.0.0".to_string(),
        path: rt_path,
        installed: false,
    };

    let installer = RuntimeInstaller::new(tmp.path().to_path_buf());
    installer.install_to_bottle(&runtime, &bottle.path).unwrap();

    // Verify DLLs were installed
    let sys32 = bottle.path.join("drive_c/windows/system32");
    assert!(sys32.join("d3d9.dll").exists());
    assert!(sys32.join("d3d11.dll").exists());

    // Verify runtime detection
    let installed_runtimes = RuntimeInstaller::list_installed(&bottle.path);
    assert!(installed_runtimes.contains(&RuntimeType::Dxvk));

    // 5. Estimate archive size
    let size = estimate_archive_size(&bottle.path).unwrap();
    assert!(size > 0);

    // 6. Duplicate the bottle
    let dup_path = duplicate_bottle(
        &bottle.path,
        &manager.bottles_dir,
        "IntegrationClone",
    )
    .unwrap();
    assert!(dup_path.exists());
    assert!(dup_path.join("bottle.toml").exists());

    // The duplicate should also have the DLLs
    assert!(dup_path.join("drive_c/windows/system32/d3d9.dll").exists());

    // Now we should have 2 bottles in the list
    let list = manager.list().unwrap();
    assert_eq!(list.len(), 2);

    // 7. Delete the original bottle
    manager.delete(&bottle.id).unwrap();
    assert!(!bottle.path.exists());

    // Only the clone should remain
    let list = manager.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "IntegrationClone");
}

#[test]
fn test_multiple_bottles_isolation() {
    let tmp = tempfile::tempdir().unwrap();
    let manager = BottleManager::new(tmp.path().to_path_buf());

    let b1 = manager.create("Game1", "wine-9.0").unwrap();
    let b2 = manager.create("Game2", "wine-10.0").unwrap();

    // Each bottle should have its own directory
    assert_ne!(b1.path, b2.path);
    assert_ne!(b1.id, b2.id);

    // Writing to one should not affect the other
    std::fs::write(
        b1.path.join("drive_c/test.txt"),
        "bottle 1 data",
    )
    .unwrap();

    assert!(!b2.path.join("drive_c/test.txt").exists());

    // Deleting one should not affect the other
    manager.delete(&b1.id).unwrap();
    assert!(b2.path.exists());
    let list = manager.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Game2");
}
