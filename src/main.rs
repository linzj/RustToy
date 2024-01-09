mod core_detection;
mod cpu_event;

use cpu_event::{CpuEvent, CpuMonitor, SpinLooper};
use num_cpus;
use std::panic;
use std::process;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

fn main() {
    // Configures the panic behavior to not only terminate the current thread but also the entire
    // process.
    panic::set_hook(Box::new(|info| {
        // Log the panic information using the `log` crate
        eprintln!("Panic occurred: {:?}", info);

        // Abort the process
        process::abort();
    }));
    // Get the total number of logical cores
    let total_cores = num_cpus::get();

    // [Windows] Get the E-cores identified by the system.
    #[cfg(windows)]
    let e_core_ids = core_detection::identify_e_cores().expect("Failed to identify E-cores.");

    // [Non-Windows] Placeholder for E-core IDs.
    #[cfg(not(windows))]
    let e_core_ids: Vec<usize> = Vec::new();

    let rest_of_cores: Vec<usize> = (0..total_cores)
        .filter(|id| !e_core_ids.contains(id))
        .collect();

    // Create monitors
    let efficiency_monitor = Arc::new(CpuMonitor::new(
        e_core_ids.clone(),
        CpuEvent::EfficiencyCoreMonitor(Vec::new()),
        true,
    ));
    let performance_monitor = Arc::new(CpuMonitor::new(
        rest_of_cores,
        CpuEvent::PerformanceCoreMonitor(Vec::new()),
        false,
    ));

    // Start the monitors
    let (sender, receiver) = mpsc::channel();
    CpuMonitor::start(efficiency_monitor.clone(), sender.clone());
    CpuMonitor::start(performance_monitor.clone(), sender.clone());

    // Create and start the SpinLooper
    let mut spin_looper = SpinLooper::new(e_core_ids);

    // Main thread event loop
    let mut last_event_time = Instant::now();
    loop {
        match receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(CpuEvent::EfficiencyCoreMonitor(consumed_cores)) => {
                println!("Efficiency cores fully consumed: {:?}", consumed_cores);
                if !performance_monitor.is_active() && efficiency_monitor.is_active() {
                    efficiency_monitor.pause();
                    performance_monitor.resume();
                    println!("Starting spin loop!");
                    spin_looper.start();
                    last_event_time = Instant::now();
                }
            }
            Ok(CpuEvent::PerformanceCoreMonitor(consumed_cores)) => {
                println!("Performance cores fully consumed: {:?}", consumed_cores);
                last_event_time = Instant::now();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let elapsed = last_event_time.elapsed();
                if elapsed >= Duration::from_secs(10)
                    && performance_monitor.is_active()
                    && !efficiency_monitor.is_active()
                {
                    println!(
                        "No events from performance cores for {:?}. Switching to efficiency cores.",
                        elapsed
                    );
                    performance_monitor.pause();
                    efficiency_monitor.resume();
                    println!("Stopping spin loop!");
                    spin_looper.stop_and_join();
                    last_event_time = Instant::now();
                } else if !performance_monitor.is_active() && efficiency_monitor.is_active() {
                    println!("No efficiency cpu is fully consume, next loop!");
                } else {
                    // If the state is not as expected, raise an error or handle it accordingly.
                    panic!("Unexpected state: performance monitor should be active and efficiency monitor should be inactive.");
                }
            }
            Err(e) => {
                // Handle other errors (e.g., channel disconnection)
                println!("Error: {:?}", e);
                break;
            }
        }
    }
}
