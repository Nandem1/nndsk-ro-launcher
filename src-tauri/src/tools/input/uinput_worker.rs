use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, SendTimeoutError, Sender, TrySendError};
use ro_tools_core::ToolsError;
use ro_tools_linux::CombatUinput;

const HIGH_QUEUE_CAPACITY: usize = 8;
const NORMAL_QUEUE_CAPACITY: usize = 32;
const KEY_PRESS_HOLD: Duration = Duration::from_millis(1);
// Give Wine/RO a visible key-up window before selecting the skill again.
// The previous click remains protected because this gap starts only when the
// next cycle begins, after the configured post-cycle delay has elapsed.
const KEY_REARM_SETTLE: Duration = Duration::from_millis(10);
// Two milliseconds was deterministic at the evdev layer, but it was shorter
// than a game frame. Let Wine/RO consume the skill edge before mouse-down.
const KEY_TO_CLICK_SETTLE: Duration = Duration::from_millis(20);
const CLICK_HOLD: Duration = Duration::from_millis(1);
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
    pub cancelled: u64,
    pub overruns: u64,
    pub dropped: u64,
    pub errors: u64,
}

impl MetricsSnapshot {
    pub fn log_line(&self, source: InputSource, final_window: bool) -> String {
        let kind = if final_window { "final" } else { "10s" };
        format!(
            "[input-metrics] backend=uinput source={} window={} samples={} period_us[p50/p95/p99]={}/{}/{} queue_us[p50/p95/p99]={}/{}/{} cycle_us[p50/p95/p99]={}/{}/{} cancelled={} overruns={} dropped={} uinput_errors={}",
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
            self.cancelled,
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
    cancelled: u64,
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
            cancelled: self.cancelled,
            overruns: self.overruns,
            dropped: self.dropped,
            errors: self.errors,
        };
        self.periods_us.clear();
        self.queue_us.clear();
        self.cycle_us.clear();
        self.cancelled = 0;
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
    next_sequence: Arc<AtomicU64>,
    spam_generation: Arc<AtomicU64>,
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
            next_sequence: Arc::new(AtomicU64::new(1)),
            spam_generation: Arc::new(AtomicU64::new(1)),
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
                    "uinput stage=spawn worker device=combined errno=none: {error}"
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
                "uinput stage=get writer device=combined errno=none: worker no preparado".into(),
            )
        })?;
        Ok(UinputWriter {
            high_tx: worker.high_tx.clone(),
            normal_tx: worker.normal_tx.clone(),
            source,
            deadline_budget,
            metrics: Arc::clone(&self.metrics),
            next_sequence: Arc::clone(&self.next_sequence),
            spam_generation: Arc::clone(&self.spam_generation),
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
    next_sequence: Arc<AtomicU64>,
    spam_generation: Arc<AtomicU64>,
}

impl UinputWriter {
    pub fn press_key(&self, key: &str) -> Result<(), ToolsError> {
        self.submit(CommandKind::PressKey(key.to_string()), None, None)
            .map(|_| ())
    }

    pub fn spam_cycle(&self, key: &str, deadline: Option<Instant>) -> Result<bool, ToolsError> {
        let cancellation = SpamCancellation {
            observed_generation: self.spam_generation.load(Ordering::Acquire),
            current_generation: Arc::clone(&self.spam_generation),
        };
        self.submit(
            CommandKind::SpamCycle(key.to_string()),
            deadline,
            Some(cancellation),
        )
    }

    pub fn release_spam(&self) -> Result<(), ToolsError> {
        // Invalidate an in-flight cycle immediately. The high-priority command
        // below then performs the idempotent physical key/mouse cleanup.
        self.spam_generation.fetch_add(1, Ordering::AcqRel);
        self.submit(CommandKind::ReleaseSpam, None, None)
            .map(|_| ())
    }

    pub fn key_event(&self, key: &str, value: i32) -> Result<(), ToolsError> {
        self.submit(CommandKind::KeyEvent(key.to_string(), value), None, None)
            .map(|_| ())
    }

    fn submit(
        &self,
        kind: CommandKind,
        absolute_deadline: Option<Instant>,
        cancellation: Option<SpamCancellation>,
    ) -> Result<bool, ToolsError> {
        let enqueued = Instant::now();
        let deadline = if matches!(kind, CommandKind::SpamCycle(_)) {
            absolute_deadline.or(Some(enqueued + self.deadline_budget))
        } else {
            None
        };
        let reliable_cleanup = matches!(&kind, CommandKind::ReleaseSpam);
        let (ack_tx, ack_rx) = bounded(1);
        let command = WorkerCommand {
            source: self.source,
            kind,
            enqueued,
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            deadline,
            cancellation,
            ack: ack_tx,
        };
        let sender = if self.source.high_priority() || reliable_cleanup {
            &self.high_tx
        } else {
            &self.normal_tx
        };
        if reliable_cleanup {
            match sender.send_timeout(command, ACK_TIMEOUT) {
                Ok(()) => {}
                Err(SendTimeoutError::Timeout(_)) => {
                    self.bump(|metrics| metrics.dropped += 1);
                    return Err(ToolsError::Other(format!(
                        "uinput cleanup queue timeout source={}",
                        self.source.as_str()
                    )));
                }
                Err(SendTimeoutError::Disconnected(_)) => {
                    self.bump(|metrics| metrics.errors += 1);
                    return Err(ToolsError::Other("uinput worker disconnected".into()));
                }
            }
        } else {
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
    ReleaseSpam,
    KeyEvent(String, i32),
}

#[derive(Default)]
struct SpamState {
    held_key: Option<String>,
    mouse_left_pressed: bool,
    cancelled_through: Option<u64>,
}

struct WorkerCommand {
    source: InputSource,
    kind: CommandKind,
    enqueued: Instant,
    sequence: u64,
    deadline: Option<Instant>,
    cancellation: Option<SpamCancellation>,
    ack: Sender<CommandOutcome>,
}

struct SpamCancellation {
    observed_generation: u64,
    current_generation: Arc<AtomicU64>,
}

impl SpamCancellation {
    fn requested(&self) -> bool {
        self.current_generation.load(Ordering::Acquire) != self.observed_generation
    }
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
    let mut spam_state = SpamState::default();
    loop {
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        let command = if let Ok(command) = high_rx.try_recv() {
            command
        } else {
            crossbeam_channel::select_biased! {
                recv(shutdown_rx) -> _ => {
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
        let outcome = execute_command(&mut device, &mut spam_state, &command, &metrics);
        let _ = command.ack.send(outcome);
    }

    let _ = release_spam_inputs(&mut device, &mut spam_state);
    device.release(None, true);
}

fn execute_command<D: InputDevice>(
    device: &mut D,
    spam_state: &mut SpamState,
    command: &WorkerCommand,
    metrics: &Metrics,
) -> CommandOutcome {
    let started = Instant::now();
    let cancelled = matches!(&command.kind, CommandKind::SpamCycle(_))
        && (spam_state
            .cancelled_through
            .is_some_and(|cutoff| command.sequence <= cutoff)
            || command
                .cancellation
                .as_ref()
                .is_some_and(SpamCancellation::requested));
    if cancelled {
        if command.source.records_metrics() {
            record_metric(metrics, command.source, |metric| metric.cancelled += 1);
        }
        return CommandOutcome::Overrun;
    }
    if command.deadline.is_some_and(|deadline| started > deadline) {
        if command.source.records_metrics() {
            record_metric(metrics, command.source, |metric| metric.overruns += 1);
        }
        return CommandOutcome::Overrun;
    }

    let result = match &command.kind {
        CommandKind::PressKey(key) => press_key(device, spam_state, key),
        CommandKind::SpamCycle(key) => {
            match spam_cycle(device, spam_state, key, command.cancellation.as_ref()) {
                Ok(true) => Ok(()),
                Ok(false) => {
                    if command.source.records_metrics() {
                        record_metric(metrics, command.source, |metric| metric.cancelled += 1);
                    }
                    return CommandOutcome::Overrun;
                }
                Err(error) => Err(error),
            }
        }
        CommandKind::ReleaseSpam => {
            // A high-priority release may overtake an older normal-queue cycle.
            // Mark the enqueue boundary so that stale cycle cannot run afterward.
            spam_state.cancelled_through = Some(
                spam_state
                    .cancelled_through
                    .map_or(command.sequence, |cutoff| cutoff.max(command.sequence)),
            );
            release_spam_inputs(device, spam_state).map(|_| ())
        }
        CommandKind::KeyEvent(key, value) => key_event(device, spam_state, key, *value),
    };
    let completed = Instant::now();
    match result {
        Ok(()) => {
            if command.source.records_metrics()
                && !matches!(
                    &command.kind,
                    CommandKind::ReleaseSpam | CommandKind::KeyEvent(..)
                )
            {
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

fn press_key(
    device: &mut impl InputDevice,
    spam_state: &mut SpamState,
    key: &str,
) -> Result<(), ToolsError> {
    let conflicts_with_spam = spam_state
        .held_key
        .as_deref()
        .is_some_and(|held| held.eq_ignore_ascii_case(key));
    if conflicts_with_spam {
        return Err(ToolsError::Input {
            key: key.to_string(),
            message: "tecla ocupada por el spammer activo".into(),
        });
    }

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

fn key_event(
    device: &mut impl InputDevice,
    spam_state: &mut SpamState,
    key: &str,
    value: i32,
) -> Result<(), ToolsError> {
    let result = device.key_event(key, value);
    if result.is_ok()
        && value == 0
        && spam_state
            .held_key
            .as_deref()
            .is_some_and(|held| held.eq_ignore_ascii_case(key))
    {
        spam_state.held_key = None;
    }
    if let Err(error) = result {
        device.release(Some(key), false);
        return Err(error);
    }
    Ok(())
}

fn spam_cycle(
    device: &mut impl InputDevice,
    spam_state: &mut SpamState,
    key: &str,
    cancellation: Option<&SpamCancellation>,
) -> Result<bool, ToolsError> {
    if release_spam_inputs(device, spam_state)? {
        // Produce a real up->down edge before every click after the first one.
        thread::sleep(KEY_REARM_SETTLE);
    }

    if cancellation.is_some_and(SpamCancellation::requested) {
        return Ok(false);
    }

    spam_state.held_key = Some(key.to_string());
    if let Err(error) = device.key_event(key, 1) {
        device.release(Some(key), true);
        return Err(error);
    }

    // Keyboard and mouse share one evdev FIFO. The settle gives Wine/RO time to
    // select the skill; the key deliberately remains down after mouse-up.
    thread::sleep(KEY_TO_CLICK_SETTLE);
    if cancellation.is_some_and(SpamCancellation::requested) {
        release_spam_inputs(device, spam_state)?;
        return Ok(false);
    }

    spam_state.mouse_left_pressed = true;
    if let Err(error) = device.mouse_left_event(1) {
        device.release(Some(key), true);
        return Err(error);
    }
    thread::sleep(CLICK_HOLD);
    if let Err(error) = device.mouse_left_event(0) {
        device.release(Some(key), true);
        return Err(error);
    }
    spam_state.mouse_left_pressed = false;
    Ok(true)
}

fn release_spam_inputs(
    device: &mut impl InputDevice,
    spam_state: &mut SpamState,
) -> Result<bool, ToolsError> {
    let had_key = spam_state.held_key.is_some();
    let mut first_error = None;

    if spam_state.mouse_left_pressed {
        match device.mouse_left_event(0) {
            Ok(()) => spam_state.mouse_left_pressed = false,
            Err(error) => first_error = Some(error),
        }
    }

    if let Some(key) = spam_state.held_key.clone() {
        match device.key_event(&key, 0) {
            Ok(()) => spam_state.held_key = None,
            Err(error) => {
                device.release(Some(&key), true);
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(had_key)
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
        cancel_on: Option<String>,
        cancellation_generation: Option<Arc<AtomicU64>>,
    }

    impl InputDevice for MockDevice {
        fn key_event(&mut self, key: &str, value: i32) -> Result<(), ToolsError> {
            let event = format!("key:{key}:{value}");
            self.events.push(event.clone());
            if self.fail_on.as_ref() == Some(&event) {
                return Err(ToolsError::Other("mock uinput write failed".into()));
            }
            if self.cancel_on.as_ref() == Some(&event) {
                if let Some(generation) = &self.cancellation_generation {
                    generation.fetch_add(1, Ordering::AcqRel);
                }
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
        static NEXT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let (ack, _) = bounded(1);
        WorkerCommand {
            source: InputSource::Spammer,
            kind,
            enqueued: Instant::now(),
            sequence: NEXT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            deadline,
            cancellation: None,
            ack,
        }
    }

    fn cancellable_command(
        kind: CommandKind,
        cancellation_generation: Arc<AtomicU64>,
    ) -> WorkerCommand {
        let observed_generation = cancellation_generation.load(Ordering::Acquire);
        let mut command = command(kind, None);
        command.cancellation = Some(SpamCancellation {
            observed_generation,
            current_generation: cancellation_generation,
        });
        command
    }

    #[test]
    fn first_spam_cycle_clicks_while_key_stays_down() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let started = Instant::now();
        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(CommandKind::SpamCycle("F2".into()), None),
            &metrics,
        );
        assert!(matches!(outcome, CommandOutcome::Completed));
        assert_eq!(device.events, ["key:F2:1", "mouse:1", "mouse:0"]);
        assert_eq!(spam_state.held_key.as_deref(), Some("F2"));
        assert!(started.elapsed() >= KEY_TO_CLICK_SETTLE + CLICK_HOLD);
    }

    #[test]
    fn repeated_spam_cycle_rearms_key_before_exactly_one_click() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let started = Instant::now();

        for _ in 0..2 {
            let outcome = execute_command(
                &mut device,
                &mut spam_state,
                &command(CommandKind::SpamCycle("F2".into()), None),
                &metrics,
            );
            assert!(matches!(outcome, CommandOutcome::Completed));
        }

        assert_eq!(
            device.events,
            ["key:F2:1", "mouse:1", "mouse:0", "key:F2:0", "key:F2:1", "mouse:1", "mouse:0",]
        );
        assert_eq!(
            device
                .events
                .iter()
                .filter(|event| *event == "mouse:1")
                .count(),
            2
        );
        assert_eq!(spam_state.held_key.as_deref(), Some("F2"));
        assert!(started.elapsed() >= KEY_REARM_SETTLE + (KEY_TO_CLICK_SETTLE + CLICK_HOLD) * 2);
    }

    #[test]
    fn physical_release_during_skill_settle_cancels_pending_click() {
        let cancellation_generation = Arc::new(AtomicU64::new(1));
        let mut device = MockDevice {
            cancel_on: Some("key:F2:1".into()),
            cancellation_generation: Some(Arc::clone(&cancellation_generation)),
            ..Default::default()
        };
        let mut spam_state = SpamState::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));

        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &cancellable_command(CommandKind::SpamCycle("F2".into()), cancellation_generation),
            &metrics,
        );

        assert!(matches!(outcome, CommandOutcome::Overrun));
        assert_eq!(device.events, ["key:F2:1", "key:F2:0"]);
        assert!(spam_state.held_key.is_none());
        assert!(!device.events.iter().any(|event| event == "mouse:1"));
        assert_eq!(
            metrics
                .lock()
                .unwrap()
                .get(&InputSource::Spammer)
                .unwrap()
                .cancelled,
            1
        );
    }

    #[test]
    fn changing_spam_key_releases_old_key_before_new_click() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        for key in ["F2", "F3"] {
            let outcome = execute_command(
                &mut device,
                &mut spam_state,
                &command(CommandKind::SpamCycle(key.into()), None),
                &metrics,
            );
            assert!(matches!(outcome, CommandOutcome::Completed));
        }

        assert_eq!(
            &device.events[3..],
            ["key:F2:0", "key:F3:1", "mouse:1", "mouse:0"]
        );
        assert_eq!(spam_state.held_key.as_deref(), Some("F3"));
    }

    #[test]
    fn release_spam_is_idempotent() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState {
            held_key: Some("F2".into()),
            ..Default::default()
        };
        let metrics = Arc::new(Mutex::new(HashMap::new()));

        for _ in 0..2 {
            let outcome = execute_command(
                &mut device,
                &mut spam_state,
                &command(CommandKind::ReleaseSpam, None),
                &metrics,
            );
            assert!(matches!(outcome, CommandOutcome::Completed));
        }

        assert_eq!(device.events, ["key:F2:0"]);
        assert!(spam_state.held_key.is_none());
    }

    #[test]
    fn expired_spam_cycle_is_skipped_without_events() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState {
            held_key: Some("F2".into()),
            ..Default::default()
        };
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(
                CommandKind::SpamCycle("F2".into()),
                Some(Instant::now() - Duration::from_millis(1)),
            ),
            &metrics,
        );
        assert!(matches!(outcome, CommandOutcome::Overrun));
        assert!(device.events.is_empty());
        assert_eq!(spam_state.held_key.as_deref(), Some("F2"));
    }

    #[test]
    fn release_spam_invalidates_an_older_queued_cycle() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let stale_cycle = command(CommandKind::SpamCycle("F2".into()), None);
        let release = command(CommandKind::ReleaseSpam, None);

        let release_outcome = execute_command(&mut device, &mut spam_state, &release, &metrics);
        let stale_outcome = execute_command(&mut device, &mut spam_state, &stale_cycle, &metrics);

        assert!(matches!(release_outcome, CommandOutcome::Completed));
        assert!(matches!(stale_outcome, CommandOutcome::Overrun));
        assert!(device.events.is_empty());

        let fresh_outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(CommandKind::SpamCycle("F2".into()), None),
            &metrics,
        );
        assert!(matches!(fresh_outcome, CommandOutcome::Completed));
        assert_eq!(device.events, ["key:F2:1", "mouse:1", "mouse:0"]);
    }

    #[test]
    fn out_of_order_releases_cannot_move_cancellation_cutoff_backwards() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let older_release = command(CommandKind::ReleaseSpam, None);
        let stale_cycle = command(CommandKind::SpamCycle("F2".into()), None);
        let newer_release = command(CommandKind::ReleaseSpam, None);

        assert!(matches!(
            execute_command(&mut device, &mut spam_state, &newer_release, &metrics),
            CommandOutcome::Completed
        ));
        assert!(matches!(
            execute_command(&mut device, &mut spam_state, &older_release, &metrics),
            CommandOutcome::Completed
        ));
        assert_eq!(spam_state.cancelled_through, Some(newer_release.sequence));
        assert!(matches!(
            execute_command(&mut device, &mut spam_state, &stale_cycle, &metrics),
            CommandOutcome::Overrun
        ));
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
        let mut spam_state = SpamState::default();
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(CommandKind::SpamCycle("F2".into()), None),
            &metrics,
        );
        assert!(matches!(outcome, CommandOutcome::Failed(_)));
        assert_eq!(device.events.last().unwrap(), "release:Some(\"F2\"):true");
        assert_eq!(spam_state.held_key.as_deref(), Some("F2"));
        assert!(spam_state.mouse_left_pressed);
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
    fn same_key_press_is_rejected_without_breaking_spam_latch() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState {
            held_key: Some("F2".into()),
            ..Default::default()
        };
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(CommandKind::PressKey("F2".into()), None),
            &metrics,
        );

        assert!(matches!(outcome, CommandOutcome::Failed(_)));
        assert!(device.events.is_empty());
        assert_eq!(spam_state.held_key.as_deref(), Some("F2"));
    }

    #[test]
    fn different_key_press_preserves_spam_latch() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState {
            held_key: Some("F2".into()),
            ..Default::default()
        };
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(CommandKind::PressKey("F8".into()), None),
            &metrics,
        );

        assert!(matches!(outcome, CommandOutcome::Completed));
        assert_eq!(device.events, ["key:F8:1", "key:F8:0"]);
        assert_eq!(spam_state.held_key.as_deref(), Some("F2"));
    }

    #[test]
    fn failed_spam_release_keeps_latch_for_cleanup_retry() {
        let mut device = MockDevice {
            fail_on: Some("key:F2:0".into()),
            ..Default::default()
        };
        let mut spam_state = SpamState {
            held_key: Some("F2".into()),
            ..Default::default()
        };
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(CommandKind::ReleaseSpam, None),
            &metrics,
        );

        assert!(matches!(outcome, CommandOutcome::Failed(_)));
        assert_eq!(spam_state.held_key.as_deref(), Some("F2"));
        assert_eq!(device.events.last().unwrap(), "release:Some(\"F2\"):true");
    }

    #[test]
    fn release_spam_cleans_possible_mouse_down_before_key() {
        let mut device = MockDevice::default();
        let mut spam_state = SpamState {
            held_key: Some("F2".into()),
            mouse_left_pressed: true,
            ..Default::default()
        };
        let metrics = Arc::new(Mutex::new(HashMap::new()));
        let outcome = execute_command(
            &mut device,
            &mut spam_state,
            &command(CommandKind::ReleaseSpam, None),
            &metrics,
        );

        assert!(matches!(outcome, CommandOutcome::Completed));
        assert_eq!(device.events, ["mouse:0", "key:F2:0"]);
        assert!(spam_state.held_key.is_none());
        assert!(!spam_state.mouse_left_pressed);
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
        const TARGET_PERIOD: Duration = Duration::from_millis(41);
        const CYCLES_PER_WINDOW: u32 = 244;

        let input = UinputInput::new();
        input.prepare().unwrap();
        let writer = input
            .writer(InputSource::Spammer, Duration::from_millis(10))
            .unwrap();

        for _window in 0..10 {
            let started = Instant::now();
            for cycle in 0..CYCLES_PER_WINDOW {
                let deadline = started + TARGET_PERIOD * cycle;
                if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                    thread::sleep(remaining);
                }
                writer.spam_cycle("F12", None).unwrap();
            }
            let metrics = input.snapshot_metrics(InputSource::Spammer);
            assert!(metrics.period_p95_us <= 44_000, "{metrics:?}");
            assert!(metrics.period_p99_us <= 48_000, "{metrics:?}");
            assert!(metrics.queue_p95_us <= 2_000, "{metrics:?}");
        }

        input.shutdown();
    }
}
