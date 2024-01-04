use core_affinity;
use std::thread;

#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_IDLE,
};

#[cfg(windows)]
mod e_cores {
    use windows::{
        Win32::{
            Foundation::{ERROR_INSUFFICIENT_BUFFER},
            System::SystemInformation::{
                GetLogicalProcessorInformationEx, RelationProcessorCore,
                SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
            },
        },
    };

    pub fn identify_e_cores() -> windows::core::Result<Vec<usize>> {
        let mut e_cores = Vec::new();
        let mut buffer_size: u32 = 0;
        let mut found_p_core = false;

        // First call to determine the size of the buffer needed.
        let result = unsafe {
            GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut buffer_size)
        };

        // If the call fails because of an insufficient buffer, we allocate and try again.
        if let Err(e) = result {
            if e.code() == ERROR_INSUFFICIENT_BUFFER.into() {
                let mut buffer = vec![0u8; buffer_size as usize];

                // Second call to actually get the data.
                let result = unsafe {
                    GetLogicalProcessorInformationEx(
                        RelationProcessorCore,
                        Some(buffer.as_mut_ptr().cast()),
                        &mut buffer_size,
                    )
                };

                if result.is_ok() {
                    let mut offset = 0;
                    while (offset as u32) < buffer_size {
                        unsafe {
                            let info = &*(buffer.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX);

                            if info.Relationship == RelationProcessorCore {
                                let processor_info = &info.Anonymous.Processor;
                                // Check if the EfficiencyClass suggests this is an E-core.
                                if processor_info.EfficiencyClass == 0 {
                                    let group_mask_ptr = processor_info.GroupMask.as_ptr();
                                    // Iterate through GROUP_AFFINITY array
                                    for i in 0..processor_info.GroupCount as isize {
                                        let group_info = &*group_mask_ptr.offset(i);
                                        // Get the affinity mask
                                        let affinity: usize = group_info.Mask; // The mask is a usize.
                                        // Identify the E-cores' logical processors
                                        for j in 0..usize::BITS { // Use `usize::BITS` to be platform-independent.
                                            if (affinity & (1 << j)) != 0 {
                                                e_cores.push(group_info.Group as usize * usize::BITS as usize + j as usize);
                                            }
                                        }
                                    }
                                } else {
                                    found_p_core = true;
                                }
                            }
                            // Move to the next entry
                            offset += info.Size as usize;
                        }
                    }
                    if !found_p_core {
                        // If no P-core was found, return an error
                        return Err(windows::core::Error::new(
                            windows::Win32::Foundation::ERROR_NOT_SUPPORTED.into(), // Convert to HRESULT
                            "not heterogeneous cpu arch".into(),
                        ));
                    }
                } else {
                    return Err(windows::core::Error::from_win32());
                }
            } else {
                return Err(e);
            }
        }

        Ok(e_cores)
    }
}

#[cfg(not(windows))]
mod e_cores {
    pub fn identify_e_cores() -> Result<Vec<usize>, String> {
        Err("identify_e_cores is not supported on non-Windows platforms.".to_string())
    }
}

fn main() {
    // [Windows] Get the E-cores identified by the system.
    #[cfg(windows)]
    let e_core_ids = e_cores::identify_e_cores().expect("Failed to identify E-cores.");

    // [Non-Windows] Placeholder for E-core IDs.
    #[cfg(not(windows))]
    let e_core_ids: Vec<usize> = Vec::new();

    // Create and spawn threads with the specified CPU affinity.
    let mut thread_handles = vec![];
    for core_id in e_core_ids {
        let handle = thread::spawn(move || {
            // Set the thread's CPU affinity to the E-core.
            core_affinity::set_for_current(core_affinity::CoreId { id: core_id });

            // Set the thread's priority to the lowest.
            set_lowest_priority();

            // Infinite loop to keep the thread alive.
            loop {
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
