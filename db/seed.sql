-- Seed data for the Cauldron game compatibility database.
-- These are well-known titles with realistic compatibility information
-- for running Windows games on macOS via the Cauldron translation stack.

INSERT INTO games (steam_app_id, title, backend, compat_status, known_issues, notes) VALUES
(1245620, 'Elden Ring', 'D3DMetal', 'Gold',
 'Occasional shader compilation stutter on first run',
 'D3D12 title. Runs well after shader cache is built. Online play functional.'),

(1086940, 'Baldur''s Gate 3', 'DXMT', 'Gold',
 'Minor shadow artifacts in Act 3 city scenes',
 'Supports both DX11 and Vulkan. DX11 via DXMT recommended for stability.'),

(1091500, 'Cyberpunk 2077', 'D3DMetal', 'Silver',
 'Ray tracing not supported; occasional crashes in dense areas',
 'D3D12 title. Playable at medium settings. Path tracing unavailable.'),

(39210, 'FINAL FANTASY XIV Online', 'DXMT', 'Platinum',
 NULL,
 'DX11 mode via DXMT. Excellent performance. All expansions tested through Dawntrail.'),

(2322010, 'God of War Ragnarok', 'D3DMetal', 'Silver',
 'Frame pacing issues during cutscenes; some texture pop-in',
 'D3D12 title. Playable but not perfectly smooth. 30fps target recommended.'),

(553850, 'HELLDIVERS 2', 'D3DMetal', 'Silver',
 'GameGuard anti-cheat may cause launch failures',
 'D3D12 title. Anti-cheat compatibility is intermittent. Single-player workaround available.'),

(1716740, 'Starfield', 'D3DMetal', 'Bronze',
 'Frequent crashes; significant performance issues; some quests broken',
 'D3D12 title. Launches but not reliably playable. Heavy GPU workload.'),

(1145360, 'Hades', 'DXMT', 'Platinum',
 NULL,
 'DX11 title. Flawless performance. Runs at high framerates on all Apple Silicon Macs.'),

(413150, 'Stardew Valley', 'DxvkMoltenVK', 'Platinum',
 NULL,
 'DX9 title via DXVK to MoltenVK. Lightweight and runs perfectly.'),

(374320, 'DARK SOULS III', 'DXMT', 'Gold',
 'Rare controller disconnect on sleep/wake',
 'DX11 title. Solid 60fps on M1 Pro and above.'),

(489830, 'The Elder Scrolls V: Skyrim Special Edition', 'DXMT', 'Gold',
 'Some ENB presets cause rendering issues',
 'DX11 title. Mod support via manual installation. SKSE works.'),

(271590, 'Grand Theft Auto V', 'DXMT', 'Gold',
 'Social Club overlay can cause hangs on launch',
 'DX11 title. Online mode functional. Good performance at 1080p.'),

(292030, 'The Witcher 3: Wild Hunt', 'DXMT', 'Gold',
 'HairWorks causes significant performance drop',
 'DX11 title. Next-gen update works. Disable HairWorks for best performance.'),

(367520, 'Hollow Knight', 'DXMT', 'Platinum',
 NULL,
 'DX11 title. Perfect performance on all Apple Silicon Macs.'),

(504230, 'Celeste', 'DXMT', 'Platinum',
 NULL,
 'DX11 title. Runs flawlessly. Input latency is excellent.'),

(1174180, 'Red Dead Redemption 2', 'D3DMetal', 'Silver',
 'Long initial load times; intermittent texture corruption in snow areas',
 'Supports DX12 and Vulkan. D3DMetal via DX12 path recommended.'),

(582010, 'MONSTER HUNTER: WORLD', 'DXMT', 'Gold',
 'Occasional disconnect in multiplayer sessions',
 'DX11 title. Good performance. Iceborne expansion tested and working.'),

(814380, 'Sekiro: Shadows Die Twice', 'DXMT', 'Gold',
 'Frame drops in some boss arenas on base M1',
 'DX11 title. Stable 60fps on M1 Pro and above.'),

(1196590, 'Resident Evil Village', 'D3DMetal', 'Gold',
 'Minor audio desync in pre-rendered cutscenes',
 'D3D12 title. Ray tracing not available but otherwise excellent.'),

(1687950, 'Persona 5 Royal', 'DXMT', 'Platinum',
 NULL,
 'DX11 title. Runs perfectly. No issues reported across full playthrough.');

-- ============================================================================
-- Game Recommended Settings (game_recommended_settings table)
-- ============================================================================

-- Sync disable games
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, msync_enabled, esync_enabled) VALUES
(7670, 0, 0);      -- BioShock 1

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, msync_enabled, esync_enabled) VALUES
(409720, 0, 0);    -- BioShock 2 Remastered

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, msync_enabled) VALUES
(638970, 0);       -- Yakuza 0

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, msync_enabled) VALUES
(834530, 0);       -- Yakuza Kiwami

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, esync_enabled) VALUES
(49520, 0);        -- Borderlands 2

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, esync_enabled) VALUES
(261640, 0);       -- Borderlands: The Pre-Sequel

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, msync_enabled, esync_enabled) VALUES
(214950, 0, 0);    -- Total War: Rome II

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, msync_enabled, esync_enabled) VALUES
(9350, 0, 0);      -- Supreme Commander

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, msync_enabled, esync_enabled) VALUES
(311730, 0, 0);    -- Dead or Alive 5

-- Windows version games
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, windows_version) VALUES
(105450, 'winxp');  -- Age of Empires 3

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, windows_version) VALUES
(211420, 'win7');   -- Dark Souls PTDE

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, windows_version) VALUES
(495420, 'win7');   -- State of Decay 2

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, windows_version) VALUES
(281280, 'winxp');  -- Mashed

-- CPU topology games
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(19900, '16:1');    -- Far Cry 2

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(220240, '24:1');   -- Far Cry 3

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(298110, '16:1');   -- Far Cry 4

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(552520, '16:1');   -- Far Cry 5

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(371660, '31:1');   -- Far Cry Primal

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(233270, '24:1');   -- Far Cry Blood Dragon

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(15620, '8:1');     -- Dawn of War II

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(94400, '8:1');     -- Prototype

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(242760, '4:1');    -- The Forest

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES
(424840, '1:1');    -- Little Nightmares

-- Vendor ID spoofing + launch args
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, env_vars, launch_args) VALUES
(1659040, '{"DXVK_CUSTOM_VENDOR_ID": "10de"}', '--skip-version-check');  -- HITMAN 3

-- Launcher bypasses (exe_override)
-- Borderlands 2 already exists from sync disable, update with exe_override
UPDATE game_recommended_settings SET exe_override = 'Binaries/Win32/Borderlands2.exe' WHERE steam_app_id = 49520;

-- Borderlands TPS already exists from sync disable, update with exe_override
UPDATE game_recommended_settings SET exe_override = 'Binaries/Win32/BorderlandsPreSequel.exe' WHERE steam_app_id = 261640;

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, exe_override) VALUES
(440900, 'ConanSandbox/Binaries/Win64/ConanSandbox.exe');  -- Conan Exiles

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, exe_override) VALUES
(700600, 'evilgenius_vulkan.exe');  -- Evil Genius 2

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, exe_override) VALUES
(312670, 'StrangeBrigade_Vulkan.exe');  -- Strange Brigade

-- Audio latency games
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, audio_latency_ms, required_dependencies) VALUES
(377840, 60, '["vcrun2019", "quartz", "lavfilters"]');  -- Final Fantasy IX

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, audio_latency_ms, required_dependencies) VALUES
(268050, 90, '["quartz"]');  -- The Evil Within

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, audio_latency_ms) VALUES
(65540, 60);   -- Gothic

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, audio_latency_ms) VALUES
(244210, 60);  -- Assetto Corsa

-- Required dependencies: AAA games
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, required_dependencies) VALUES
(1593500, '["vcrun2022", "d3dcompiler_47"]');  -- God of War

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, required_dependencies) VALUES
(1151640, '["vcrun2019"]');  -- Horizon Zero Dawn

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, required_dependencies) VALUES
(1817070, '["vcrun2022", "d3dcompiler_47"]');  -- Spider-Man Remastered

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, required_dependencies) VALUES
(1293830, '["vcrun2019"]');  -- Forza Horizon 4

-- Media codec games
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, required_dependencies) VALUES
(418370, '["quartz", "lavfilters"]');  -- Resident Evil 7

INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, required_dependencies) VALUES
(631510, '["quartz", "lavfilters"]');  -- DMC HD Collection

-- ============================================================================
-- D3DMetal DXR / MetalFX game entries (GPTK 3.0+ / M3+ ray tracing)
-- ============================================================================

-- New game entries for D3DMetal DXR titles
-- Schema: games (steam_app_id, exe_hash, title, backend, compat_status, wine_overrides, known_issues, last_tested, notes)
INSERT OR REPLACE INTO games (steam_app_id, title, backend, compat_status, known_issues, notes) VALUES
(2215430, 'Ghost of Tsushima', 'D3DMetal', 'Silver', 'Frame pacing issues; DXR on M3+ only', 'DX12. D3DMetal required.'),
(2118960, 'Alan Wake 2', 'D3DMetal', 'Silver', 'Very GPU-heavy; DXR on M3+ only', 'DX12. D3DMetal required. Epic primary.'),
(1895880, 'Ratchet & Clank: Rift Apart', 'D3DMetal', 'Silver', 'Frame drops during dimension shifts', 'DX12. MetalFX recommended for DLSS replacement.'),
(1888930, 'The Last of Us Part I', 'D3DMetal', 'Silver', 'CPU-bound in open areas; shader stutter', 'DX12. D3DMetal required.'),
(1649240, 'Returnal', 'D3DMetal', 'Silver', 'UE5 demanding; DXR on M3+', 'DX12. D3DMetal required.');

-- D3DMetal DXR recommended settings
-- Schema: game_recommended_settings (steam_app_id, msync_enabled, esync_enabled, rosetta_x87,
--   async_shader, metalfx_upscaling, dxr_ray_tracing, fsr_enabled, large_address_aware,
--   wine_dll_overrides, env_vars, windows_version, launch_args, auto_apply_patches)
INSERT OR REPLACE INTO game_recommended_settings (steam_app_id, dxr_ray_tracing, env_vars) VALUES
(2215430, 1, '{"D3DM_SUPPORT_DXR": "1"}'),
(2118960, 1, '{"D3DM_SUPPORT_DXR": "1"}'),
(1895880, 0, '{"D3DM_ENABLE_METALFX": "1"}'),
(1888930, 0, '{}'),
(1649240, 1, '{"D3DM_SUPPORT_DXR": "1"}'),
(870780,  1, '{"D3DM_SUPPORT_DXR": "1"}');
