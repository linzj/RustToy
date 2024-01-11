use core_affinity::CoreId;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Condvar, Mutex,
};
use std::thread::{self, JoinHandle};

#[cfg(windows)]
use windows::{
    core::PCWSTR,
    Win32::Foundation::ERROR_SUCCESS,
    Win32::System::Performance::{
        PdhAddCounterW, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
        PDH_CALC_NEGATIVE_DENOMINATOR, PDH_CALC_NEGATIVE_VALUE, PDH_CSTATUS_NEW_DATA,
        PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_INVALID_ARGUMENT,
        PDH_INVALID_DATA,
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
    active: Arc<(Mutex<bool>, Condvar)>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl CpuMonitor {
    pub fn new(cores_to_monitor: Vec<usize>, event_type: CpuEvent, active: bool) -> Self {
        CpuMonitor {
            cores_to_monitor,
            event_type,
            active: Arc::new((Mutex::new(active), Condvar::new())),
            worker: None.into(),
        }
    }

    #[cfg(windows)]
    pub fn start(self: Arc<Self>, sender: mpsc::Sender<CpuEvent>) {
        let self_clone = self.clone();
        let thread_name = self.get_thread_name();
        let worker = thread::Builder::new().name(thread_name).spawn(move || {
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
                    self.wait_for_active();

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
                        // This cpu has been shut down.
                        if status == PDH_CALC_NEGATIVE_VALUE
                            || status == PDH_CALC_NEGATIVE_DENOMINATOR
                        {
                            continue;
                        }

                        if status == PDH_INVALID_ARGUMENT {
                            eprintln!(
                                "Invalid argument for counter index {}, core id: {}.",
                                counter_handles_index - 1,
                                core_index
                            );
                            continue;
                        }

                        if status == PDH_INVALID_DATA {
                            eprintln!(
                                "Invalid data for counter index {}, core id: {}.",
                                counter_handles_index - 1,
                                core_index
                            );
                            continue;
                        }

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
        });
        let mut worker_guard = self_clone.worker.lock().unwrap();
        *worker_guard = Some(worker.unwrap());
    }

    fn wait_for_active(&self) {
        let (lock, cvar) = &*self.active;
        let mut active = lock.lock().unwrap();
        while !*active {
            // Wait for the condition variable to be notified
            active = cvar.wait(active).unwrap();
        }
    }

    fn get_thread_name(&self) -> String {
        match self.event_type {
            CpuEvent::EfficiencyCoreMonitor(_) => "EfficiencyCoreMonitor_thread".to_string(),
            CpuEvent::PerformanceCoreMonitor(_) => "PerformanceCoreMonitor_thread".to_string(),
        }
    }

    // Methods to control the active state of the monitor
    pub fn pause(&self) {
        let (lock, _cvar) = &*self.active;
        let mut active = lock.lock().unwrap();
        *active = false;
    }

    pub fn resume(&self) {
        let (lock, cvar) = &*self.active;
        let mut active = lock.lock().unwrap();
        *active = true;
        cvar.notify_one();
    }

    pub fn is_active(&self) -> bool {
        let (lock, _cvar) = &*self.active;
        let active = lock.lock().unwrap();
        *active
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
            let should_stop = self.should_stop.clone();
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
        self.should_stop.store(false, Ordering::SeqCst);
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
