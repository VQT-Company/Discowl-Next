@echo off
setlocal

REM === Visual Studio 2026 Developer Environment ===
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"

REM === Add LLVM/Clang to PATH ===
set PATH=C:\Program Files\LLVM\bin;%PATH%

REM === MozTools for SpiderMonkey build ===
set MOZTOOLS_PATH=%USERPROFILE%\moztools-4.0\moztools-4.0
set PATH=%MOZTOOLS_PATH%\bin;%MOZTOOLS_PATH%\msys2\usr\bin;%PATH%

REM === LLVM/Clang for bindgen & mozjs ===
set LIBCLANG_PATH=C:\Program Files\LLVM\lib
set CC=clang-cl
set CXX=clang-cl
set LD=lld-link

REM === Python ===
set PYTHON=C:\Python314\python.exe
set PYTHON3=C:\Python314\python.exe

REM === Increase Servo style thread stack to 8MB (default 512KB overflows on Windows) ===
set SERVO_STYLE_THREAD_STACK_SIZE_KB=8192

REM === Build with single job to avoid clang OOM ===
cargo build -j1 %*
