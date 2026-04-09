#!/usr/bin/env python3
"""Apply the safe VirtualProtect COW patch to Wine 11.6.

This version only adds:
1. VPROT_WRITTEN flag definition
2. VPROT_WRITTEN preservation in set_vprot
3. COW write tracking in virtual_handle_fault

It does NOT change:
- get_unix_prot (WRITECOPY pages stay writable — changing this broke wineboot)
- map_file_into_view (MAP_SHARED stays on macOS — changing this broke wineboot)
"""
import sys

wine_dir = sys.argv[1] if len(sys.argv) > 1 else "/tmp/clean-wine/wine"
path = f"{wine_dir}/dlls/ntdll/unix/virtual.c"

with open(path) as f:
    content = f.read()

changes = 0

# 1. Add VPROT_WRITTEN flag
old = "#define VPROT_WRITEWATCH 0x40\n"
new = "#define VPROT_WRITEWATCH 0x40\n#define VPROT_WRITTEN    0x80  /* page was COW-written; private copy exists */\n"
if old in content and "VPROT_WRITTEN" not in content:
    content = content.replace(old, new)
    changes += 1
    print("  1. Added VPROT_WRITTEN flag")

# 2. Preserve VPROT_WRITTEN in set_vprot
old2 = "static BOOL set_vprot( struct file_view *view, void *base, size_t size, BYTE vprot )\n{"
new2 = (
    "static BOOL set_vprot( struct file_view *view, void *base, size_t size, BYTE vprot )\n"
    "{\n"
    "    /* Cauldron: preserve VPROT_WRITTEN across protection changes */\n"
    "    BYTE old_vprot = get_host_page_vprot( base );\n"
    "    if (old_vprot & VPROT_WRITTEN) vprot |= VPROT_WRITTEN;\n"
)
if old2 in content:
    content = content.replace(old2, new2)
    changes += 1
    print("  2. Added VPROT_WRITTEN preservation in set_vprot")

# 3. Add fault handler for WRITECOPY tracking
marker = "        /* ignore fault if page is writable now */"
if marker in content and "Cauldron: track COW" not in content:
    handler = (
        "        /* Cauldron: track COW writes on WRITECOPY pages so VirtualProtect\n"
        "         * doesn't discard private pages when restoring read-only protection. */\n"
        "        if ((vprot & VPROT_WRITECOPY) && !(vprot & VPROT_WRITTEN))\n"
        "        {\n"
        "            set_page_vprot_bits( page, host_page_size, VPROT_WRITE | VPROT_WRITTEN, VPROT_WRITECOPY );\n"
        "            mprotect_range( page, host_page_size, 0, 0 );\n"
        "            ret = STATUS_SUCCESS;\n"
        "        }\n"
        "        else if ((vprot & VPROT_WRITTEN) && !(get_unix_prot( vprot ) & PROT_WRITE))\n"
        "        {\n"
        "            set_page_vprot_bits( page, host_page_size, VPROT_WRITE, 0 );\n"
        "            mprotect_range( page, host_page_size, 0, 0 );\n"
        "            ret = STATUS_SUCCESS;\n"
        "        }\n"
        "        /* ignore fault if page is writable now */"
    )
    content = content.replace(marker, handler)
    changes += 1
    print("  3. Added COW fault handler")

with open(path, "w") as f:
    f.write(content)

print(f"\nApplied {changes} changes (safe version — no get_unix_prot or MAP_SHARED changes)")
