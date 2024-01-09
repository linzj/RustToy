use core_affinity::CoreId;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(windows)]
use windows::{
    core::PCWSTR,
    Win32::Foundation::ERROR_SUCCESS,
    Win32::System::Performance::{
        PdhAddCounterW, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
        PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
    },
};
// Define an enum for the events we are interested in
pub enum CpuEvent {
    EfficiencyCoreMonitor(Vec<usize>),
    PerformanceCoreMonitor(Vec<usize>),
}

pub struct CpuMonitor {
    cores_to_monitor: Vec<usize>,
    event_type: CpuEvent,
    active: Arc<AtomicBool>,
}

impl CpuMonitor {
    pub fn new(
        cores_to_monitor: Vec<usize>,
        event_type: CpuEvent,
        active: Arc<AtomicBool>,
    ) -> Self {
        CpuMonitor {
            cores_to_monitor,
            event_type,
            active,
        }
    }

    #[cfg(windows)]
    pub fn start(self: Arc<Self>, sender: mpsc::Sender<CpuEvent>) -> JoinHandle<()> {
        thread::spawn(move || {
            // Using PDH to monitor CPU usage
            unsafe {
                // Open a query
                let mut query: isize = 0;
                let mut status = PdhOpenQueryW(PCWSTR::null(), 0, &mut query);
                if status != ERROR_SUCCESS.0 {
                    panic!("PdhOpenQueryW failed with error: {}", status);
                }

                // Buffer to hold the counter paths for each core
                let mut counter_handles: Vec<isize> = Vec::new();

                // Create a counter for each core
                for &core_index in &self.cores_to_monitor {
                    let counter_path = format!(
                        r"\Processor Information(0,{})\% Processor Utility",
                        core_index
                    );
                    let mut counter_handle: isize = 0;
                    let wide_counter_path =
                        widestring::U16CString::from_str(&counter_path).unwrap();
                    status = PdhAddCounterW(
                        query,
                        PCWSTR(wide_counter_path.as_ptr()),
                        0,
                        &mut counter_handle,
                    );
                    if status != ERROR_SUCCESS.0 {
                        panic!("PdhAddCounterW failed with error: {}", status);
                    }
                    counter_handles.push(counter_handle);
                }

                loop {
                    if !self.active.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(500));
                        continue;
                    }

                    // Collect the query data
                    status = PdhCollectQueryData(query);
                    if status != ERROR_SUCCESS.0 {
                        panic!("PdhCollectQueryData failed with error: {:x}", status);
                    }

                    // Wait for a second to have a time sample
                    std::thread::sleep(std::time::Duration::from_secs(1));

                    // Collect the second set of data
                    status = PdhCollectQueryData(query);
                    if status != ERROR_SUCCESS.0 {
                        panic!("PdhCollectQueryData failed with error: {:x}", status);
                    }
                    // Retrieve and process the calculated counter value for each core
                    let mut fully_consumed_cores = Vec::new();
                    let mut counter_handles_index = 0;
                    for &core_index in &self.cores_to_monitor {
                        let mut counter_value: PDH_FMT_COUNTERVALUE =
                            PDH_FMT_COUNTERVALUE::default();
                        status = PdhGetFormattedCounterValue(
                            counter_handles[counter_handles_index],
                            PDH_FMT_DOUBLE,
                            Some(std::ptr::null_mut()),
                            &mut counter_value,
                        );
                        counter_handles_index += 1;
                        if status != ERROR_SUCCESS.0 {
                            panic!(
                                "PdhGetFormattedCounterValue failed with error: {:x}",
                                status
                            );
                        }
                        if counter_value.CStatus == PDH_CSTATUS_VALID_DATA
                            || counter_value.CStatus == PDH_CSTATUS_NEW_DATA
                        {
                            let value = counter_value.Anonymous.doubleValue;
                            if value >= 100.0 {
                                fully_consumed_cores.push(core_index);
                            }
                        }
                    }

                    // Send event if there are fully consumed cores
                    if !fully_consumed_cores.is_empty() {
                        let event = match &self.event_type {
                            CpuEvent::EfficiencyCoreMonitor(_) => {
                                CpuEvent::EfficiencyCoreMonitor(fully_consumed_cores)
                            }
                            CpuEvent::PerformanceCoreMonitor(_) => {
                                CpuEvent::PerformanceCoreMonitor(fully_consumed_cores)
                            }
                        };
                        if let Err(e) = sender.send(event) {
                            // Handle error (e.g., the receiver might have been dropped)
                            // For simplicity, we panic here, but you may want to handle it more gracefully
                            panic!("Failed to send CpuEvent: {}", e);
                        }
                    }
                }
                // status = PdhCloseQuery(query);
                // if status != ERROR_SUCCESS.0 {
                //     panic!("PdhCloseQuery failed with error: {}", status);
                // }
            }
        })
    }

    // Methods to control the active state of the monitor
    pub fn pause(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.active.store(true, Ordering::SeqCst);
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}

pub struct SpinLooper {
    core_ids: Vec<usize>,
    handles: Vec<JoinHandle<()>>,
    should_stop: Arc<AtomicBool>,
}

impl SpinLooper {
    // Build a new SpinLooper with the given core IDs, but do not start the threads yet.
    pub fn new(core_ids: Vec<usize>) -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        SpinLooper {
            core_ids,
            handles: Vec::new(),
            should_stop,
        }
    }

    // Start the spin loop on each core in a separate thread.
    pub fn start(&mut self) {
        assert!(self.handles.is_empty(), "SpinLooper already started.");

        for &core_id in &self.core_ids {
            let should_stop = Arc::clone(&self.should_stop);
            let handle = thread::spawn(move || {
                // Set the thread's CPU affinity to the specified core.
                core_affinity::set_for_current(CoreId { id: core_id });
                set_lowest_priority();

                // Spin loop until told to stop.
                while !should_stop.load(Ordering::SeqCst) {
                    std::hint::spin_loop();
                }
            });
            self.handles.push(handle);
        }
    }

    // Stop all the spinning threads and join them.
    pub fn stop_and_join(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            handle.join().expect("Failed to join SpinLooper thread");
        }
    }
}

#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_IDLE,
};

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
