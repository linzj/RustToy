use crate::cpu_event::{CpuEvent, CpuMonitor, SpinLooper};
use std::error::Error;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Define a trait for core states
trait CoreState {
    fn handle(&self, controller: &mut CoreStateController) -> Result<(), Box<dyn Error>>;
}

// Define the state controller
pub struct CoreStateController {
    receiver: Receiver<CpuEvent>,
    efficiency_monitor: Arc<CpuMonitor>,
    performance_monitor: Arc<CpuMonitor>,
    spin_looper: SpinLooper,
    current_state: Rc<Box<dyn CoreState>>,
    last_event_time: Instant,
}

impl CoreStateController {
    pub fn new(e_core_ids: Vec<usize>, rest_of_cores: Vec<usize>) -> Self {
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
        let spin_looper = SpinLooper::new(e_core_ids);
        CoreStateController {
            receiver,
            efficiency_monitor,
            performance_monitor,
            spin_looper,
            current_state: Rc::new(Box::new(ECoreState)),
            last_event_time: Instant::now(),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let current_state = &self.current_state.clone();
            current_state.handle(self)?;
        }
    }

    fn switch_to_ecore_state(&mut self) {
        println!("Switching to ECoreState");
        self.performance_monitor.pause();
        self.efficiency_monitor.resume();
        self.spin_looper.stop_and_join();
        self.current_state = Rc::new(Box::new(ECoreState));
    }

    fn switch_to_pcore_state(&mut self) {
        println!("Switching to PCoreState");
        self.efficiency_monitor.pause();
        self.performance_monitor.resume();
        self.spin_looper.start();
        self.current_state = Rc::new(Box::new(PCoreState));
    }
}

// Define the ECoreState
struct ECoreState;

impl CoreState for ECoreState {
    fn handle(&self, controller: &mut CoreStateController) -> Result<(), Box<dyn Error>> {
        match controller.receiver.recv() {
            // Wait indefinitely for the event
            Ok(CpuEvent::EfficiencyCoreMonitor(consumed_cores)) => {
                println!("Efficiency cores fully consumed: {:?}", consumed_cores);
                controller.switch_to_pcore_state();
            }
            Ok(_) => {
                // Ignore other events in this state
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        }
        Ok(())
    }
}

// Define the PCoreState
struct PCoreState;

impl CoreState for PCoreState {
    fn handle(&self, controller: &mut CoreStateController) -> Result<(), Box<dyn Error>> {
        match controller.receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(CpuEvent::PerformanceCoreMonitor(consumed_cores)) => {
                println!("Performance cores fully consumed: {:?}", consumed_cores);
                controller.last_event_time = Instant::now();
            }
            Ok(CpuEvent::EfficiencyCoreMonitor(consumed_cores)) => {
                println!(
                    "Efficiency cores fully consumed in PCoreState: {:?}",
                    consumed_cores
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let elapsed = controller.last_event_time.elapsed();
                if elapsed >= Duration::from_secs(10) {
                    controller.switch_to_ecore_state();
                }
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        }
        Ok(())
    }
}
