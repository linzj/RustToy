# RustCpuDeadLoop

RustCpuDeadLoop is a Rust application that manages CPU loads to optimize process scheduling on systems with Efficient (E) and Performance (P) cores. The application monitors CPU usage, generating load on E cores only when they are fully utilized to encourage the scheduler to assign new tasks to P cores. It reverts to a low-impact monitoring state when P cores are underutilized to prevent system strain.

## Requirements

To run this application, you need the Rust toolchain installed on your system. You can install Rust using rustup, which is available at https://rustup.rs/.

## Usage

Run the compiled application:

```sh
cargo run --release
```

The application monitors and manages CPU load without further input.

## How It Works

Identifies E and P cores.
Monitors E core consumption.
Applies load on E cores when fully consumed.
Ceases load and monitors P cores to avoid overuse.

## Platform Support

Specialized for Windows E core identification; other platforms may differ.

## Dependencies

num_cpus
core_affinity
widestring (Windows only)
Windows Performance Counters (Windows only)

## Note

This application is designed for testing and demonstration purposes. It will create an infinite load on the system's CPU cores, which can affect the performance of other running tasks. Use it with caution and understand that it can make the system less responsive, particularly if incorrectly configured to occupy all available cores.

## License

This application is distributed under the MIT license, which allows for modification, distribution, and private use.
