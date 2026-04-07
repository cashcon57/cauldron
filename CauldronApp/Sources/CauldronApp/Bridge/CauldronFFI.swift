import Foundation

// C function declarations matching cauldron-bridge exports.
// These are resolved at link time against libcauldron_bridge.a.

@_silgen_name("cauldron_init")
func cauldron_init(_ base_dir: UnsafePointer<CChar>?) -> UnsafeMutableRawPointer?

@_silgen_name("cauldron_free")
func cauldron_free(_ ptr: UnsafeMutableRawPointer?)

@_silgen_name("cauldron_create_bottle")
func cauldron_create_bottle(
    _ mgr: UnsafeMutableRawPointer?,
    _ name: UnsafePointer<CChar>?,
    _ wine_version: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_list_bottles")
func cauldron_list_bottles(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_delete_bottle")
func cauldron_delete_bottle(
    _ mgr: UnsafeMutableRawPointer?,
    _ id: UnsafePointer<CChar>?
) -> Int32

@_silgen_name("cauldron_init_db")
func cauldron_init_db(_ path: UnsafePointer<CChar>?) -> Int32

@_silgen_name("cauldron_query_game")
func cauldron_query_game(
    _ mgr: UnsafeMutableRawPointer?,
    _ appId: UInt32
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_list_games")
func cauldron_list_games(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_get_sync_status")
func cauldron_get_sync_status(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_get_wine_versions")
func cauldron_get_wine_versions(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_scan_bottle_games")
func cauldron_scan_bottle_games(
    _ mgr: UnsafeMutableRawPointer?,
    _ bottleId: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_discover_bottles")
func cauldron_discover_bottles(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_import_bottle")
func cauldron_import_bottle(
    _ mgr: UnsafeMutableRawPointer?,
    _ source_path: UnsafePointer<CChar>?,
    _ name: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_run_sync")
func cauldron_run_sync(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_launch_exe")
func cauldron_launch_exe(
    _ mgr: UnsafeMutableRawPointer?,
    _ bottle_id: UnsafePointer<CChar>?,
    _ exe_path: UnsafePointer<CChar>?,
    _ backend: UnsafePointer<CChar>?
) -> Int32

@_silgen_name("cauldron_download_wine")
func cauldron_download_wine(
    _ mgr: UnsafeMutableRawPointer?,
    _ version: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_get_installed_wine")
func cauldron_get_installed_wine(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_get_proton_commits")
func cauldron_get_proton_commits(
    _ mgr: UnsafeMutableRawPointer?,
    _ filter: UnsafePointer<CChar>?,
    _ limit: UInt32
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_apply_patch")
func cauldron_apply_patch(
    _ mgr: UnsafeMutableRawPointer?,
    _ hash: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_skip_patch")
func cauldron_skip_patch(
    _ mgr: UnsafeMutableRawPointer?,
    _ hash: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_reverse_patch")
func cauldron_reverse_patch(
    _ mgr: UnsafeMutableRawPointer?,
    _ hash: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_analyze_patches")
func cauldron_analyze_patches(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_verify_build")
func cauldron_verify_build(_ mgr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cauldron_free_string")
func cauldron_free_string(_ s: UnsafeMutablePointer<CChar>?)
