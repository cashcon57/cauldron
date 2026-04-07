#!/bin/bash
# Build KosmicKrisp (Mesa Vulkan driver for Metal) from source
# Requires: macOS 26+, Apple Silicon, Xcode CLI tools
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

MESA_VERSION="${1:-main}"
BUILD_DIR="${PROJECT_ROOT}/build/kosmickrisp"
INSTALL_DIR="${PROJECT_ROOT}/build/kosmickrisp-install"
MESA_DIR="${PROJECT_ROOT}/build/mesa-src"

echo "=== Building KosmicKrisp (Mesa Vulkan on Metal) ==="
echo "Mesa version: ${MESA_VERSION}"
echo "Project root: ${PROJECT_ROOT}"

# ---------------------------------------------------------------------------
# Step 1: Install dependencies via Homebrew
# ---------------------------------------------------------------------------
echo "[1/6] Installing build dependencies..."
brew install meson ninja cmake pkg-config llvm spirv-tools spirv-llvm-translator libclc python3 2>/dev/null || true

# Set up LLVM paths (Homebrew LLVM, not Xcode's)
LLVM_PREFIX="$(brew --prefix llvm)"
export PATH="${LLVM_PREFIX}/bin:$PATH"
export CC="${LLVM_PREFIX}/bin/clang"
export CXX="${LLVM_PREFIX}/bin/clang++"
export LDFLAGS="-L${LLVM_PREFIX}/lib -Wl,-rpath,${LLVM_PREFIX}/lib"
export CPPFLAGS="-I${LLVM_PREFIX}/include"

# Verify LLVM version (need 20.1+)
LLVM_VER=$(llvm-config --version)
echo "   LLVM version: ${LLVM_VER}"
LLVM_MAJOR=$(echo "${LLVM_VER}" | cut -d. -f1)
if [ "${LLVM_MAJOR}" -lt 20 ]; then
    echo "WARNING: LLVM ${LLVM_VER} detected. KosmicKrisp may require LLVM 20.1+."
    echo "         Consider: brew install llvm@20"
fi

# ---------------------------------------------------------------------------
# Step 2: Clone/update Mesa source
# ---------------------------------------------------------------------------
echo "[2/6] Getting Mesa source..."
mkdir -p "$(dirname "${MESA_DIR}")"

if [ -d "${MESA_DIR}/.git" ]; then
    echo "   Updating existing Mesa checkout..."
    cd "${MESA_DIR}"
    git fetch --all
    git checkout "${MESA_VERSION}" 2>/dev/null || git checkout "origin/${MESA_VERSION}" 2>/dev/null || true
    cd "${PROJECT_ROOT}"
else
    echo "   Cloning Mesa (this may take a while)..."
    if ! git clone --depth=1 --branch="${MESA_VERSION}" \
        https://gitlab.freedesktop.org/mesa/mesa.git "${MESA_DIR}" 2>/dev/null; then
        echo "   Branch '${MESA_VERSION}' not found, cloning default branch..."
        git clone --depth=1 https://gitlab.freedesktop.org/mesa/mesa.git "${MESA_DIR}"
    fi
fi

# Verify the source contains KosmicKrisp
if [ ! -d "${MESA_DIR}/src/kosmickrisp" ] && [ ! -d "${MESA_DIR}/src/vulkan/drivers/kosmickrisp" ]; then
    echo "WARNING: KosmicKrisp driver source not found in expected locations."
    echo "   Checked: src/kosmickrisp, src/vulkan/drivers/kosmickrisp"
    echo "   The driver may be under a different path in this Mesa version."
    echo "   Listing Vulkan drivers available:"
    ls -d "${MESA_DIR}"/src/*/vulkan 2>/dev/null || true
    ls -d "${MESA_DIR}"/src/vulkan/drivers/* 2>/dev/null || true
fi

# ---------------------------------------------------------------------------
# Step 3: Configure with Meson
# ---------------------------------------------------------------------------
echo "[3/6] Configuring Mesa build..."
cd "${MESA_DIR}"

# Remove old build directory if reconfigure is needed
if [ -d "${BUILD_DIR}" ] && [ -f "${BUILD_DIR}/build.ninja" ]; then
    echo "   Reconfiguring existing build directory..."
    meson setup "${BUILD_DIR}" \
        --reconfigure \
        --buildtype=release \
        --prefix="${INSTALL_DIR}" \
        -Dplatforms=macos \
        -Dvulkan-drivers=kosmickrisp \
        -Dgallium-drivers= \
        -Dopengl=false \
        -Degl=disabled \
        -Dglx=disabled \
        -Dzstd=disabled \
        -Dllvm=enabled \
        -Dshared-llvm=disabled \
        --prefer-static \
        2>&1 || { echo "ERROR: Meson reconfiguration failed"; exit 1; }
else
    meson setup "${BUILD_DIR}" \
        --buildtype=release \
        --prefix="${INSTALL_DIR}" \
        -Dplatforms=macos \
        -Dvulkan-drivers=kosmickrisp \
        -Dgallium-drivers= \
        -Dopengl=false \
        -Degl=disabled \
        -Dglx=disabled \
        -Dzstd=disabled \
        -Dllvm=enabled \
        -Dshared-llvm=disabled \
        --prefer-static \
        2>&1 || { echo "ERROR: Meson configuration failed"; exit 1; }
fi

# ---------------------------------------------------------------------------
# Step 4: Build
# ---------------------------------------------------------------------------
echo "[4/6] Building (this may take 10-20 minutes)..."
NPROC=$(sysctl -n hw.logicalcpu 2>/dev/null || echo 4)
ninja -C "${BUILD_DIR}" -j"${NPROC}"

# ---------------------------------------------------------------------------
# Step 5: Install
# ---------------------------------------------------------------------------
echo "[5/6] Installing to ${INSTALL_DIR}..."
ninja -C "${BUILD_DIR}" install

cd "${PROJECT_ROOT}"

# ---------------------------------------------------------------------------
# Step 6: Verify
# ---------------------------------------------------------------------------
echo "[6/6] Verifying installation..."

# Search for the ICD JSON in several potential locations
ICD_JSON=""
for candidate in \
    "${INSTALL_DIR}/share/vulkan/icd.d" \
    "${INSTALL_DIR}/etc/vulkan/icd.d" \
    "${INSTALL_DIR}/lib/vulkan/icd.d"; do
    if [ -d "${candidate}" ]; then
        found=$(find "${candidate}" -name "*kosmickrisp*" -o -name "*kosmic*" 2>/dev/null | head -1)
        if [ -n "${found}" ]; then
            ICD_JSON="${found}"
            break
        fi
    fi
done

if [ -z "${ICD_JSON}" ]; then
    # Broader search
    ICD_JSON=$(find "${INSTALL_DIR}" -name "*kosmickrisp*.json" 2>/dev/null | head -1)
fi

if [ -n "${ICD_JSON}" ]; then
    echo "=== KosmicKrisp ICD found: ${ICD_JSON} ==="
    echo ""
    echo "ICD contents:"
    cat "${ICD_JSON}"
    echo ""
else
    echo "WARNING: KosmicKrisp ICD JSON not found in ${INSTALL_DIR}"
    echo "   All installed files matching 'kosmic':"
    find "${INSTALL_DIR}" -name "*kosmic*" 2>/dev/null || echo "   (none)"
    echo ""
    echo "   All ICD JSON files:"
    find "${INSTALL_DIR}" -name "*.json" -path "*/vulkan/*" 2>/dev/null || echo "   (none)"
    exit 1
fi

# Check Vulkan extensions if vulkaninfo is available
if command -v vulkaninfo &>/dev/null && [ -n "${ICD_JSON}" ]; then
    echo ""
    echo "=== Vulkan Extension Check ==="

    ICD_DIR="$(dirname "${ICD_JSON}")"
    EXTENSIONS=$(VK_DRIVER_FILES="${ICD_JSON}" vulkaninfo 2>/dev/null || true)

    if [ -n "${EXTENSIONS}" ]; then
        echo "${EXTENSIONS}" | grep -i "apiVersion" | head -1 || true

        if echo "${EXTENSIONS}" | grep -qi "VK_EXT_graphics_pipeline_library"; then
            echo "VK_EXT_graphics_pipeline_library: SUPPORTED"
            echo "*** DXVK 2.x should work with KosmicKrisp! ***"
        else
            echo "VK_EXT_graphics_pipeline_library: NOT FOUND"
            echo "*** DXVK 2.x may be blocked without this extension ***"
        fi

        if echo "${EXTENSIONS}" | grep -qi "VK_EXT_transform_feedback"; then
            echo "VK_EXT_transform_feedback: SUPPORTED"
        else
            echo "VK_EXT_transform_feedback: NOT FOUND"
        fi

        if echo "${EXTENSIONS}" | grep -qi "VK_EXT_extended_dynamic_state2"; then
            echo "VK_EXT_extended_dynamic_state2: SUPPORTED"
        else
            echo "VK_EXT_extended_dynamic_state2: NOT FOUND"
        fi
    else
        echo "Could not query Vulkan extensions (vulkaninfo failed)."
        echo "This is expected if no GPU is available in this environment."
    fi
fi

echo ""
echo "============================================"
echo "  KosmicKrisp build complete!"
echo "============================================"
echo ""
echo "ICD JSON:  ${ICD_JSON}"
echo ""
echo "To use KosmicKrisp with DXVK, set:"
echo "  export VK_DRIVER_FILES=${ICD_JSON}"
echo ""
echo "To test with vulkaninfo:"
echo "  VK_DRIVER_FILES=${ICD_JSON} vulkaninfo --summary"
echo ""
echo "To use with Cauldron:"
echo "  cauldron kk status"
