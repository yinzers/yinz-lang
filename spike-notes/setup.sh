#!/bin/sh
# Rebuilds the no-root toolchain under $ROOT (default: <worktree>/.spike-env). Idempotent.
set -e
ROOT=${ROOT:-$(cd "$(dirname "$0")/.." && pwd)/.spike-env}
mkdir -p $ROOT/debs $ROOT/root $ROOT/shim $ROOT/cargo $ROOT/rustup $ROOT/asan-bin
export CARGO_HOME=$ROOT/cargo RUSTUP_HOME=$ROOT/rustup
if [ ! -x $CARGO_HOME/bin/rustc ]; then
  curl -sSf https://sh.rustup.rs -o $ROOT/rustup.sh
  sh $ROOT/rustup.sh -y --profile minimal --default-toolchain stable >$ROOT/rustup.log 2>&1
fi
if [ ! -x $ROOT/root/usr/lib/llvm-18/bin/clang-18 ]; then
  cd $ROOT/debs
  for f in clang-18 libclang-common-18-dev libclang-cpp18 libclang-rt-18-dev libclang1-18 libllvm18 llvm-18-dev llvm-18-runtime llvm-18 libclang-18-dev; do
    curl -sSfO http://deb.debian.org/debian/pool/main/l/llvm-toolchain-18/${f}_18.1.8-18+b1_amd64.deb
  done
  for d in *.deb; do dpkg -x $d $ROOT/root; done
fi
L=/usr/lib/x86_64-linux-gnu
ln -sf $L/libffi.so.8 $ROOT/shim/libffi.so
ln -sf $L/libz.so.1 $ROOT/shim/libz.so
ln -sf $L/libzstd.so.1 $ROOT/shim/libzstd.so
ln -sf $L/libxml2.so.2 $ROOT/shim/libxml2.so
ln -sf $L/libstdc++.so.6 $ROOT/shim/libstdc++.so
cat > $ROOT/asan-bin/clang-18 <<EOS
#!/bin/sh
exec $ROOT/root/usr/lib/llvm-18/bin/clang-18 -fsanitize=address -g "\$@"
EOS
chmod +x $ROOT/asan-bin/clang-18
WT=$(cd "$(dirname "$0")/.." && pwd)
cat > $ROOT/env.sh <<EOS
export CARGO_HOME=$ROOT/cargo RUSTUP_HOME=$ROOT/rustup
export PATH=$ROOT/cargo/bin:$ROOT/root/usr/lib/llvm-18/bin:$WT/target/debug:\$PATH
export LLVM_SYS_181_PREFIX=$ROOT/root/usr/lib/llvm-18
export LD_LIBRARY_PATH=$ROOT/root/usr/lib/llvm-18/lib:$ROOT/root/usr/lib/x86_64-linux-gnu:$ROOT/shim
export LIBRARY_PATH=$ROOT/shim:$ROOT/root/usr/lib/llvm-18/lib
export ASAN_BIN=$ROOT/asan-bin
EOS
echo done
