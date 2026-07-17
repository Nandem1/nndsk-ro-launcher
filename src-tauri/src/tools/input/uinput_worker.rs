use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use ro_tools_core::ToolsError;
use ro_tools_linux::CombatUinput;

const HIGH_QUEUE_CAPACITY: usize = 8;
const NORMAL_QUEUE_CAPACITY: usize = 32;
const KEY_PRESS_HOLD: Duration = Duration::from_millis(1);
const KEY_TO_CLICK_SETTLE: Duration = Duration::from_millis(2);
const CLICK_HOLD: Duration = Duration::from_millis(1);
const CLICK_TO_KEY_RELEASE_SETTLE: Duration = Duration::from_millis(1);
const ACK_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputSource {
    Autopot,
    Autobuff,
    Spammer,
    Gear,
}

impl InputSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Autopot => "autopot",
            Self::Autobuff => "autobuff",
            Self::Spammer => "spammer",
            Self::Gear => "gear",
        }
    }

    fn high_priority(self) -> bool {
        matches!(self, Self::Autopot)
    }

    fn records_metrics(self) -> bool {
        matches!(self, Self::Autopot | Self::Spammer)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    pub samples: usize,
    pub period_p50_us: u64,
    pub period_p95_us: u64,
    pub period_p99_us: u64,
    pub queue_p50_us: u64,
    pub queue_p95_us: u64,
    pub queue_p99_us: u64,
    pub cycle_p50_us: u64,
    pub cycle_p95_us: u64,
    pub cycle_p99_us: u64,
    pub overruns: u64,
    pub dropped: u64,
    pub errors: u64,
}

impl MetricsSnapshot {
    pub fn log_line(&self, source: InputSource, final_window: bool) -> String {
        let kind = if final_window { "final" } else { "10s" };
        format!(
            "[input-metrics] backend=uinput source={} window={} samples={} period_us[p50/p95/p99]={}/{}/{} queue_us[p50/p95/p99]={}/{}/{} cycle_us[p50/p95/p99]={}/{}/{} overruns={} dropped={} uinput_errors={}",
            source.as_str(),
            kind,
            self.samples,
            self.period_p50_us,
            self.period_p95_us,
            self.period_p99_us,
            self.queue_p50_us,
            self.queue_p95_us,
            self.queue_p99_us,
            self.cycle_p50_us,
            self.cycle_p95_us,
            self.cycle_p99_us,
            self.overruns,
            self.dropped,
            self.errors,
        )
    }
}

#[derive(Default)]
struct SourceMetrics {
    periods_us: Vec<u64>,
    queue_us: Vec<u64>,
    cycle_us: Vec<u64>,
    last_start: Option<Instant>,
    overruns: u64,
    dropped: u64,
    errors: u64,
}

impl SourceMetrics {
    fn record_completed(&mut self, started: Instant, queue: Duration, cycle: Duration) {
        if let Some(previous) = self.last_start.replace(started) {
            self.periods_us
                .push(duration_us(started.duration_since(previous)));
        }
        self.queue_us.push(duration_us(queue));
        self.cycle_us.push(duration_us(cycle));
    }

    fn snapshot_and_reset(&mut self) -> MetricsSnapshot {
        let snapshot = MetricsSnapshot {
            samples: self.cycle_us.len(),
            period_p50_us: percentile(&self.periods_us, 50),
            period_p95_us: percentile(&self.periods_us, 95),
            period_p99_us: percentile(&self.periods_us, 99),
            queue_p50_us: percentile(&self.queue_us, 50),
            queue_p95_us: percentile(&self.queue_us, 95),
            queue_p99_us: percentile(&self.queue_us, 99),
            cycle_p50_us: percentile(&self.cycle_us, 50),
            cycle_p95_us: percentile(&self.cycle_us, 95),
            cycle_p99_us: percentile(&self.cycle_us, 99),
            overruns: self.overruns,
            dropped: self.dropped,
            errors: self.errors,
        };
        self.periods_us.clear();
        self.queue_us.clear();
        self.cycle_us.clear();
        self.overruns = 0;
        self.dropped = 0;
        self.errors = 0;
        snapshot
    }
}

type Metrics = Arc<Mutex<HashMap<InputSource, SourceMetrics>>>;

#[derive(Clone)]
pub struct UinputInput {
    inner: Arc<Mutex<Option<WorkerHandle>>>,
    metrics: Metrics,
}

struct WorkerHandle {
    high_tx: Sender<WorkerCommand>,
    normal_tx: Sender<WorkerCommand>,
    shutdown_tx: Sender<()>,
    join: Option<JoinHandle<()>>,
    device_summary: String,
}

impl UinputInput {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            metrics: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn prepare(&self) -> Result<String, ToolsError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| ToolsError::Other("uinput worker lock poisoned".into()))?;
        if let Some(worker) = guard.as_ref() {
            return Ok(worker.device_summary.clone());
        }

        let device = CombatUinput::create()?;
        let device_summary = device.device_summary();
        let (high_tx, high_rx) = bounded(HIGH_QUEUE_CAPACITY);
        let (normal_tx, normal_rx) = bounded(NORMAL_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_rx) = bounded(1);
        let metrics = Arc::clone(&self.metrics);
        let join = thread::Builder::new()
            .name("ro-uinput-worker".into())
            .spawn(move || worker_loop(device, high_rx, normal_rx, shutdown_rx, metrics))
            .map_err(|error| {
                ToolsError::Other(format!(
                    "uinput stage=spawn worker device=both errno=none: {error}"
                ))
            })?;

        *guard = Some(WorkerHandle {
            high_tx,
            normal_tx,
            shutdown_tx,
            join: Some(join),
            device_summary: device_summary.clone(),
        });
        Ok(device_summary)
    }

    pub fn is_prepared(&self) -> bool {
        self.inner
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    pub fn writer(
        &self,
        source: InputSource,
        deadline_budget: Duration,
    ) -> Result<UinputWriter, ToolsError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| ToolsError::Other("uinput worker lock poisoned".into()))?;
        let worker = guard.as_ref().ok_or_else(|| {
            ToolsError::Other(
                "uinput stage=get writer device=both errno=none: worker no preparado".into(),
            )
        })?;
        Ok(UinputWriter {
            high_tx: worker.high_tx.clone(),
            normal_tx: worker.normal_tx.clone(),
            source,
            deadline_budget,
            metrics: Arc::clone(&self.metrics),
        })
    }

    pub fn snapshot_metrics(&self, source: InputSource) -> MetricsSnapshot {
        self.metrics
            .lock()
            .ok()
            .and_then(|mut metrics| {
                metrics
                    .get_mut(&source)
                    .map(SourceMetrics::snapshot_and_reset)
            })
            .unwrap_or_default()
    }

    pub fn shutdown(&self) {
        let Some(mut worker) = self.inner.lock().ok().and_then(|mut guard| guard.take()) else {
            return;
        };
        let _ = worker.shutdown_tx.send(());
        if let Some(join) = worker.join.take() {
            let _ = join.join();
        }
    }
}

impl Default for UinputInput {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct UinputWriter {
    high_tx: Sender<WorkerCommand>,
    normal_tx: Sender<WorkerCommand>,
    source: InputSource,
    deadline_budget: Duration,
    metrics: Metrics,
}

impl UinputWriter {
    pub fn press_key(&self, key: &str) -> Result<(), ToolsError> {
        self.submit(CommandKind::PressKey(key.to_string()), None)
            .map(|_| ())
    }

    pub fn spam_cycle(&self, key: &str, deadline: Option<Instant>) -> Result<bool, ToolsError> {
        self.submit(CommandKind::SpamCycle(key.to_string()), deadline)
    }

    pub fn key_event(&self, key: &str, value: i32) -> Result<(), ToolsError> {
        self.submit(CommandKind::KeyEvent(key.to_string(), value), None)
            .map(|_| ())
    }

    fn submit(
        &self,
        kind: CommandKind,
        absolute_deadline: Option<Instant>,
    ) -> Result<bool, ToolsError> {
        let enqueued = Instant::now();
        let deadline = if matches!(kind, CommandKind::SpamCycle(_)) {
            absolute_deadline.or(Some(enqueued + self.deadline_budget))
        } else {
            None
        };
        let (ack_tx, ack_rx) = bounded(1);
        let command = WorkerCommand {
            source: self.source,
            kind,
            enqueued,
            deadline,
            ack: ack_tx,
        };
        let sender = if self.source.high_priority() {
            &self.high_tx
        } else {
            &self.normal_tx
        };
        match sender.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.bump(|metrics| metrics.dropped += 1);
                return Err(ToolsError::Other(format!(
                    "uinput queue full source={}",
                    self.source.as_str()
                )));
            }
            Err(TrySendError::Disconnected(_)) => {
                self.bump(|metrics| metrics.errors += 1);
                return Err(ToolsError::Other("uinput worker disconnected".into()));
            }
        }

        match ack_rx.recv_timeout(ACK_TIMEOUT) {
            Ok(CommandOutcome::Completed) => Ok(true),
            Ok(CommandOutcome::Overrun) => Ok(false),
            Ok(CommandOutcome::Failed(error)) => Err(ToolsError::Other(error)),
            Err(error) => {
                self.bump(|metrics| metrics.errors += 1);
                Err(ToolsError::Other(format!("uinput worker ack: {error}")))
            }
        }
    }

    fn bump(&self, update: impl FnOnce(&mut SourceMetrics)) {
        if !self.source.records_metrics() {
            return;
        }
        if let Ok(mut metrics) = self.metrics.lock() {
            update(metrics.entry(self.source).or_default());
        }
    }
}

enum CommandKind {
    PressKey(String),
    SpamCycle(String),
    KeyEvent(String, i32),
}

struct WorkerCommand {
    source: InputSource,
    kind: CommandKind,
    enqueued: Instant,
    deadline: Option<Instant>,
    ack: Sender<CommandOutcome>,
}

enum CommandOutcome {
    Completed,
    Overrun,
    Failed(String),
}

trait InputDevice {
    fn key_event(&mut self, key: &str, value: i32) -> Result<(), ToolsError>;
    fn mouse_left_event(&mut self, value: i32) -> Result<(), ToolsError>;
    fn release(&mut self, key: Option<&str>, mouse_left: bool);
}

impl InputDevice for CombatUinput {
    fn key_event(&mut self, key: &str, value: i32) -> Result<(), ToolsError> {
        CombatUinput::key_event(self, key, value)
    }

    fn mouse_left_event(&mut self, value: i32) -> Result<(), ToolsError> {
        CombatUinput::mouse_left_event(self, value)
    }

    fn release(&mut self, key: Option<&str>, mouse_left: bool) {
        CombatUinput::release(self, key, mouse_left)
    }
}

fn worker_loop(
    mut device: CombatUinput,
    high_rx: Receiver<WorkerCommand>,
    normal_rx: Receiver<WorkerCommand>,
    shutdown_rx: Receiver<()>,
    metrics: Metrics,
) {
    loop {
        if shutdown_rx.try_recv().is_ok() {
            device.release(None, true);
            break;
        }

        let command = if let Ok(command) = high_rx.try_recv() {
            command
        } else {
            crossbeam_channel::select_biased! {
                recv(shutdown_rx) -> _ => {
                    device.release(None, true);
                    break;
                },
                recv(high_rx) -> command => match command {
                    Ok(command) => command,
                    Err(_) => break,
                },
                recv(normal_rx) -> command => match command {
                    Ok(command) => command,
                    Err(_) => break,
                },
            }
        };
        let outcome = execute_command(&mut device, &command, &metrics);
        let _ = command.ack.send(outcome);
    }
}

fn execute_command<D: InputDevice>(
    device: &mut D,
    command: &WorkerCommand,
    metrics: &Metrics,
) -> CommandOutcome {
    let started = Instant::now();
    if command.deadline.is_some_and(|deadline| started > deadline) {
        if command.source.records_metrics() {
            record_metric(metrics, command.source, |metric| metric.overruns += 1);
        }
        return CommandOutcome::Overrun;
    }

    let result = match &command.kind {
        CommandKind::PressKey(key) => press_key(device, key),
        CommandKind::SpamCycle(key) => spam_cycle(device, key),
        CommandKind::KeyEvent(key, value) => device.key_event(key, *value),
    };
    let completed = Instant::now();
    match result {
        Ok(()) => {
            if command.source.records_metrics() {
                record_metric(metrics, command.source, |metric| {
                    metric.record_completed(
                        started,
                        started.duration_since(command.enqueued),
                        completed.duration_since(started),
                    )
                });
            }
            CommandOutcome::Completed
        }
        Err(error) => {
            if command.source.records_metrics() {
                record_metric(metrics, command.source, |metric| metric.errors += 1);
            }
            CommandOutcome::Failed(error.to_string())
        }
    }
}

fn press_key(device: &mut impl InputDevice, key: &str) -> Result<(), ToolsError> {
    if let Err(error) = device.key_event(key, 1) {
        device.release(Some(key), false);
        return Err(error);
    }
    thread::sleep(KEY_PRESS_HOLD);
    if let Err(error) = device.key_event(key, 0) {
        device.release(Some(key), false);
        return Err(error);
    }
    Ok(())
}

fn spam_cycle(device: &mut impl InputDevice, key: &str) -> Result<(), ToolsError> {
    if let Err(error) = device.key_event(key, 1) {
        device.release(Some(key), true);
        return Err(error);
    }

    // Keyboard and mouse are separate virtual devices. Give Wine time to observe
    // the skill key before the click reaches RO, then keep the key pressed briefly
    // after mouse-up so cross-device delivery cannot make the release overtake it.
    thread::sleep(KEY_TO_CLICK_SETTLE);
    if let Err(error) = device.mouse_left_event(1) {
        device.release(Some(key), true);
        return Err(error);
    }
    thread::sleep(CLICK_HOLD);
    if let Err(error) = device.mouse_left_event(0) {
        device.release(Some(key), true);
        return Err(error);
    }
    thread::sleep(CLICK_TO_KEY_RELEASE_SETTLE);
    if let Err(error) = device.key_event(key, 0) {
        device.release(Some(key), false);
        return Err(error);
    }
    Ok(())
}

fn record_metric(metrics: &Metrics, source: InputSource, update: impl FnOnce(&mut SourceMetrics)) {
    if let Ok(mut guard) = metrics.lock() {
        update(guard.entry(source).or_default());
    }
}

fn duration_us(duration: Duration) -> u64 {
    duration.as_micros().min(u64::MAX as u128) as u64
}

fn percentile(values: &[u64], percentile: usize) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) * percentile).div_ceil(100);
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockDevice {
        events: Vec<String>,
        fail_on: Option<String>,
    }

    impl InputDevice for MockDevice {
        fn key_event(&mut self, key: &str, value: i32) -> Result<(), ToolsError> {
            let event = format!("key:{key}:{value}");
            self.events.push(event.clone());
            if self.fail_on.as_ref() == Some(&event) {
                return Err(ToolsError::Other("mock uinput write failed".into()));
            }
            Ok(())
        }

        fn mouse_left_event(&mut self, value: i32) -> Result<(), ToolsError> {
            let event = format!("mouse:{value}");
            self.events.push(event.clone());
            if self.fail_on.as_ref() == Some(&event) {
                return Err(ToolsError::Other("mock uinput write failed".into()));
            }
            Ok(())
        }

        fn release(&mut self, key: Option<&str>, mouse_left: bool) {
            self.events.push(format!("release:{key:?}:{mouse_left}"));
        }
    }

    fn command(kind: CommandKind, deadline: Option<Instant>) -> WorkerCommand {
        let (ack, _) = bounded(1);
        WorkerCommand {
            source: InputSource::Spammer,
            kind,
            enqueued: Instant::now(),
            deadline,
            ack,
        }
    }

    #[test]
    fn spam_cycle_has_atomic_event_order() {
        let mut device = MockDevice::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let started = Instant::now();
        let outcome = execute_command(
            &mut device,
            &command(CommandKind::SpamCycle("F2".into()), None),
            &metrics,
        );
        assert!(matches!(outcome, CommandOutcome::Completed));
        assert_eq!(
            device.events,
            ["key:F2:1", "mouse:1", "mouse:0", "key:F2:0"]
        );
        assert!(
            started.elapsed() >= KEY_TO_CLICK_SETTLE + CLICK_HOLD + CLICK_TO_KEY_RELEASE_SETTLE
        );
    }

    #[test]
    fn expired_spam_cycle_is_skipped_without_events() {
        let mut device = MockDevice::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &command(
                CommandKind::SpamCycle("F2".into()),
                Some(Instant::now() - Duration::from_millis(1)),
            ),
            &metrics,
        );
        assert!(matches!(outcome, CommandOutcome::Overrun));
        assert!(device.events.is_empty());
    }

    #[test]
    fn ready_high_priority_command_is_taken_first() {
        let (high_tx, high_rx) = bounded(2);
        let (normal_tx, normal_rx) = bounded(2);
        high_tx
            .send(command(CommandKind::PressKey("F8".into()), None))
            .unwrap();
        normal_tx
            .send(command(CommandKind::SpamCycle("F2".into()), None))
            .unwrap();
        let first = high_rx
            .try_recv()
            .or_else(|_| normal_rx.try_recv())
            .unwrap();
        assert!(matches!(first.kind, CommandKind::PressKey(_)));
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        assert_eq!(percentile(&[10, 20, 30, 40, 50], 95), 50);
        assert_eq!(percentile(&[], 95), 0);
    }

    #[test]
    fn write_error_releases_pressed_inputs_and_is_counted() {
        let mut device = MockDevice {
            fail_on: Some("mouse:1".into()),
            ..Default::default()
        };
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &command(CommandKind::SpamCycle("F2".into()), None),
            &metrics,
        );
        assert!(matches!(outcome, CommandOutcome::Failed(_)));
        assert_eq!(device.events.last().unwrap(), "release:Some(\"F2\"):true");
        assert_eq!(
            metrics
                .lock()
                .unwrap()
                .get(&InputSource::Spammer)
                .unwrap()
                .errors,
            1
        );
    }

    #[test]
    fn writer_fails_explicitly_when_worker_was_not_prepared() {
        let error = UinputInput::new()
            .writer(InputSource::Autopot, Duration::from_millis(10))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("stage=get writer"));
        assert!(error.contains("worker no preparado"));
    }

    /// Manual Linux benchmark. It injects F12 + left-click for 100 seconds;
    /// run only in a safe RO test session with access to /dev/uinput.
    #[test]
    #[ignore = "manual /dev/uinput latency benchmark"]
    fn linux_uinput_ten_consecutive_ten_second_windows() {
        let input = UinputInput::new();
        input.prepare().unwrap();
        let writer = input
            .writer(InputSource::Spammer, Duration::from_millis(10))
            .unwrap();

        for _window in 0..10 {
            let started = Instant::now();
            for cycle in 0..1000u32 {
                let deadline = started + Duration::from_millis(cycle as u64 * 10);
                if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                    thread::sleep(remaining);
                }
                writer.spam_cycle("F12", None).unwrap();
            }
            let metrics = input.snapshot_metrics(InputSource::Spammer);
            assert!(metrics.period_p95_us <= 12_000, "{metrics:?}");
            assert!(metrics.period_p99_us <= 15_000, "{metrics:?}");
            assert!(metrics.queue_p95_us <= 2_000, "{metrics:?}");
        }

        input.shutdown();
    }
}
