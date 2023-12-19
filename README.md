# RustCpuDeadLoop

This Rust application aims to generate load on the Efficient (E) cores of a multi-core system by creating busy threads. The primary goal is to fill up the E cores with work so that new processes are preferentially scheduled to the Performance (P) cores. This is particularly useful when you want to ensure that a certain task or process runs on a P core for improved performance.

## Requirements

To run this application, you need the Rust toolchain installed on your system. You can install Rust using rustup, which is available at https://rustup.rs/.

## Usage

To use the application, you need to compile it and then run it with a command-line argument specifying the starting CPU index. The application creates a busy loop on the CPU cores starting from the specified index to the last one.

cargo run -- <Starting CPU index>
Replace <Starting CPU index> with the index of the CPU from which you want to start creating busy loops. The CPU index should be a value between 0 and the number of logical cores on your system minus one.

For example, if you want to start occupying cores from CPU index 4 (assuming your system has at least 5 logical cores), you would run:

cargo run -- 4

## How It Works

When the application starts, it parses the command-line argument to determine the starting CPU index.
Then, it retrieves the available CPU core IDs and checks if the starting index is within a valid range.
The application spawns a series of threads, each with its CPU affinity set to one of the core IDs starting from the specified index.
Each thread runs an infinite loop to keep the CPU core busy.
Threads are set to the lowest scheduling priority to minimize the impact on other system tasks.
Platform Support
This application supports both Unix-like and Windows systems. Platform-specific code is used to set the thread's scheduling priority:

On Unix-like systems, it uses the libc::setpriority function.
On Windows systems, it calls the SetThreadPriority function with THREAD_PRIORITY_IDLE.

## Dependencies

The application depends on the core_affinity crate for setting thread affinity and uses platform-specific APIs to adjust thread priorities.

## Note

This application is designed for testing and demonstration purposes. It will create an infinite load on the system's CPU cores, which can affect the performance of other running tasks. Use it with caution and understand that it can make the system less responsive, particularly if incorrectly configured to occupy all available cores.

## License

This application is distributed under the MIT license, which allows for modification, distribution, and private use.
