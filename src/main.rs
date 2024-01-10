mod control;
mod core_detection;
mod cpu_event;

use crate::control::CoreStateController;
use num_cpus;
use std::panic;
use std::process;

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

    let mut core_state_controller = CoreStateController::new(e_core_ids, rest_of_cores);
    match core_state_controller.run() {
        Err(e) => {
            panic!("An error occurred while running the loop: {:?}", e);
        }
        Ok(_) => {
            panic!("Unexpected normal return from the loop!");
        }
    }
}
