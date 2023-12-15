use core_affinity;
use std::thread;

#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_IDLE,
};

fn main() {
    // Parse the CPU index from the command line argument.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <Starting CPU index>", args[0]);
        std::process::exit(1);
    }
    let start_cpu_index = args[1].parse::<usize>().expect("Invalid CPU index");

    // Get the available CPU cores.
    let core_ids = core_affinity::get_core_ids().unwrap();

    // Check if starting CPU index is within valid range.
    if start_cpu_index >= core_ids.len() {
        eprintln!(
            "Invalid starting CPU index. Must be between 0 and {}.",
            core_ids.len() - 1
        );
        std::process::exit(1);
    }

    // Create and spawn threads with the specified CPU affinity.
    let mut thread_handles = vec![];
    for core_id in core_ids.iter().skip(start_cpu_index) {
        let core_id_clone = *core_id; // Copy core_id to move into the thread.
        let handle = thread::spawn(move || {
            // Set the thread's CPU affinity.
            core_affinity::set_for_current(core_id_clone);

            // Set the thread's priority.
            set_lowest_priority();

            loop {
                // Infinite loop to keep the thread alive.
                // In a real application, you would do actual work here.
                std::hint::spin_loop();
            }
        });
        thread_handles.push(handle);
    }

    // Wait for all threads to finish (which will never happen due to the infinite loop).
    for handle in thread_handles {
        let _ = handle.join();
    }
}

#[cfg(unix)]
fn set_lowest_priority() {
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 19);
    }
}

#[cfg(windows)]
fn set_lowest_priority() {
    unsafe {
        let current_thread = GetCurrentThread();
        let _ = SetThreadPriority(current_thread, THREAD_PRIORITY_IDLE);
    }
}
