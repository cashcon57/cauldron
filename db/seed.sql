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
