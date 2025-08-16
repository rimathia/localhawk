#!/bin/bash

# Build script for iOS static library

set -e  # Exit on error

echo "🏗️  Building LocalHawk for iOS..."

# Add iOS targets if not already added
echo "📱 Adding iOS targets..."
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim

# Install cargo-lipo if not installed (for universal binary)
if ! command -v cargo-lipo &> /dev/null; then
    echo "📦 Installing cargo-lipo..."
    cargo install cargo-lipo
fi

# Clean previous builds
echo "🧹 Cleaning previous builds..."
cargo clean -p localhawk-core

# Build for all iOS targets
echo "🔨 Building for aarch64-apple-ios (device)..."
cargo build --release --target aarch64-apple-ios -p localhawk-core

echo "🔨 Building for x86_64-apple-ios (simulator x86_64)..."
cargo build --release --target x86_64-apple-ios -p localhawk-core

echo "🔨 Building for aarch64-apple-ios-sim (simulator arm64)..."
cargo build --release --target aarch64-apple-ios-sim -p localhawk-core

# Create output directory
mkdir -p ios-libs

# Create universal library for simulator (x86_64 + arm64)
echo "🔗 Creating universal simulator library..."
lipo -create \
  target/x86_64-apple-ios/release/liblocalhawk_core.a \
  target/aarch64-apple-ios-sim/release/liblocalhawk_core.a \
  -output ios-libs/liblocalhawk_core_sim.a

# Copy device library
echo "📋 Copying device library..."
cp target/aarch64-apple-ios/release/liblocalhawk_core.a ios-libs/liblocalhawk_core_device.a

# Copy header file
echo "📋 Copying header file..."
cp localhawk-core/include/localhawk.h ios-libs/

echo "✅ Build complete!"
echo ""
echo "📁 Output files:"
echo "   ios-libs/liblocalhawk_core_device.a  (for physical devices)"
echo "   ios-libs/liblocalhawk_core_sim.a     (for simulator)"
echo "   ios-libs/localhawk.h                 (header file)"
echo ""
echo "🎯 Next steps:"
echo "   1. Open the existing Xcode project: open LocalHawkiOS/LocalHawkiOS.xcodeproj"
echo "   2. Clean build folder: ⌘+Shift+K"
echo "   3. Build the project: ⌘+B"
echo "   4. Test in simulator or deploy to device"