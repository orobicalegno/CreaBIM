//! Performance diagnostics (Help › Performance diagnostics, see
//! docs/prestazioni): what each frame cost while the user works, kept in
//! memory, summed up in a small panel, and saved as a report to send.
//!
//! Off by default. Off, it costs one atomic load per timed section
//! ([`crate::app::Perf`]) and per frame: nothing is kept.
//!
//! What is measured here and what is not:
//! - measured: the time between frames, the CPU time of each frame (eframe's
//!   own figure: the UI, tessellation and rendering commands, without the
//!   wait for the screen), the timed sections, rebuilds, input events;
//! - estimated: the wait for the screen (VSync, frames queued) as the time
//!   between frames less the CPU time, while frames follow one another;
//! - not available: GPU time (the renderer does not report it to Volumen),
//!   and the latency from the hand on the mouse to light on the monitor,
//!   which only an outside measurement can give.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

static ON: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Tests that turn diagnostics on or off take this first: one at a time.
#[cfg(test)]
pub static ONE: Mutex<()> = Mutex::new(());

/// Frames kept for the panel: about a minute at 60 fps.
const KEEP: usize = 3600;
/// A recording's length, in seconds.
pub const RECORD_SECONDS: f64 = 30.0;
/// Frames are not part of one interaction when, besides the earlier
/// frame's own work, more than this passed between them: the app was idle
/// in between, as it should be when nothing changes. A slow frame is not
/// a pause: its own time does not count towards this.
const IDLE_GAP_MS: f64 = 250.0;

/// Are diagnostics on?
#[inline]
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Turn diagnostics on (from nothing kept) or off (everything dropped).
pub fn set_enabled(on: bool) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    *s = on.then(State::new);
    ON.store(on, Ordering::Relaxed);
}

/// One frame as diagnostics saw it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// When it began, in seconds since diagnostics were turned on.
    pub t: f64,
    /// Since the frame before began, in ms (waits included); `None` for
    /// the first.
    pub interval_ms: Option<f64>,
    /// `App::ui`, measured here, in ms.
    pub ui_ms: f64,
    /// The whole frame's CPU time as eframe measures it (UI, tessellation,
    /// rendering commands; not the wait for the screen), in ms. Known one
    /// frame later.
    pub cpu_ms: Option<f64>,
    /// Input events this frame.
    pub events: u32,
    pub pointer_moved: bool,
    pub buttons_down: bool,
    /// Something follows the pointer or waits for it: a move, a drag, a
    /// tool's point.
    pub working: bool,
    /// The model changed during the frame.
    pub model_changed: bool,
    /// The timed sections of the frame: (name, ms, times).
    pub sections: Vec<(&'static str, f64, u32)>,
}

impl Frame {
    /// Part of an interaction: input came, a button is held, or a tool is
    /// at work.
    pub fn interactive(&self) -> bool {
        self.events > 0 || self.buttons_down || self.working
    }

    /// Work with nothing moving: no input, no button held, yet the model
    /// was changed again (a move re-applied with the mouse still).
    pub fn idle_update(&self) -> bool {
        self.events == 0 && !self.buttons_down && self.model_changed
    }

    fn section(&self, name: &str) -> Option<(f64, u32)> {
        self.sections
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, ms, n)| (*ms, *n))
    }
}

struct State {
    start: Instant,
    frames: VecDeque<Frame>,
    /// The frame being made, by the thread making it (the UI thread):
    /// sections timed on other threads are not part of it.
    current: Option<(Instant, std::thread::ThreadId, Frame)>,
    last_begin: Option<Instant>,
    recording: Option<(f64, Vec<Frame>)>,
}

impl State {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            frames: VecDeque::new(),
            current: None,
            last_begin: None,
            recording: None,
        }
    }
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    if !enabled() {
        return None;
    }
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.as_mut().map(f)
}

/// A timed section ended (from [`crate::app::Perf`]).
pub fn section(name: &'static str, ms: f64) {
    with(|s| {
        if let Some((_, by, f)) = &mut s.current {
            if *by != std::thread::current().id() {
                return;
            }
            match f.sections.iter_mut().find(|(n, _, _)| *n == name) {
                Some((_, t, k)) => {
                    *t += ms;
                    *k += 1;
                }
                None => f.sections.push((name, ms, 1)),
            }
        }
    });
}

/// Something happened that has no duration of its own (a drawing sent to
/// a worker): counted as a section of 0 ms.
pub fn count(name: &'static str) {
    section(name, 0.0);
}

/// Whether a section is a count (no time of its own to show).
pub fn is_count(name: &str) -> bool {
    name.starts_with("worker:")
}

/// A frame begins: what input it has, and eframe's CPU time of the frame
/// before (which belongs to that frame).
pub fn frame_begin(events: u32, pointer_moved: bool, buttons_down: bool, prev_cpu_s: Option<f32>) {
    with(|s| {
        let now = Instant::now();
        if let Some(f) = s.frames.back_mut() {
            if f.cpu_ms.is_none() {
                f.cpu_ms = prev_cpu_s.map(|c| f64::from(c) * 1000.0);
            }
        }
        if let Some(r) = s.recording.as_mut().and_then(|(_, r)| r.last_mut()) {
            if r.cpu_ms.is_none() {
                r.cpu_ms = prev_cpu_s.map(|c| f64::from(c) * 1000.0);
            }
        }
        let f = Frame {
            t: (now - s.start).as_secs_f64(),
            interval_ms: s.last_begin.map(|b| (now - b).as_secs_f64() * 1000.0),
            events,
            pointer_moved,
            buttons_down,
            ..Default::default()
        };
        s.last_begin = Some(now);
        s.current = Some((now, std::thread::current().id(), f));
    });
}

/// The frame's UI is done.
pub fn frame_end(working: bool, model_changed: bool) {
    with(|s| {
        let Some((began, _, mut f)) = s.current.take() else {
            return;
        };
        f.ui_ms = began.elapsed().as_secs_f64() * 1000.0;
        f.working = working;
        f.model_changed = model_changed;
        if let Some((from, r)) = &mut s.recording {
            if f.t - *from <= RECORD_SECONDS {
                r.push(f.clone());
            }
        }
        s.frames.push_back(f);
        while s.frames.len() > KEEP {
            s.frames.pop_front();
        }
    });
}

/// Start a recording of [`RECORD_SECONDS`].
pub fn start_recording() {
    with(|s| {
        let now = s.start.elapsed().as_secs_f64();
        s.recording = Some((now, vec![]));
    });
}

/// Seconds left of the recording, if one is going on.
pub fn recording_left() -> Option<f64> {
    with(|s| {
        let (from, _) = s.recording.as_ref()?;
        Some((RECORD_SECONDS - (s.start.elapsed().as_secs_f64() - from)).max(0.0))
    })
    .flatten()
}

/// The recording, once its time is up (or now, with `now`), taken out.
pub fn take_recording(now: bool) -> Option<Vec<Frame>> {
    with(|s| {
        let (from, _) = s.recording.as_ref()?;
        if !now && s.start.elapsed().as_secs_f64() - from < RECORD_SECONDS {
            return None;
        }
        s.recording.take().map(|(_, r)| r)
    })
    .flatten()
}

/// The frames of the last `seconds`.
pub fn recent(seconds: f64) -> Vec<Frame> {
    with(|s| {
        let now = s.start.elapsed().as_secs_f64();
        s.frames
            .iter()
            .filter(|f| now - f.t <= seconds)
            .cloned()
            .collect()
    })
    .unwrap_or_default()
}

/// Median, 95th and 99th percentile and maximum of `v`; `None` when empty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub n: usize,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

pub fn spread(v: &[f64]) -> Option<Spread> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let q = |p: f64| s[((s.len() - 1) as f64 * p).round() as usize];
    Some(Spread {
        n: s.len(),
        p50: q(0.5),
        p95: q(0.95),
        p99: q(0.99),
        max: s[s.len() - 1],
    })
}

/// The timed sections the panel and the report show, grouped, with what
/// each one is. Times include the sections inside them.
pub const SECTIONS: &[(&str, &str)] = &[
    (
        "edit: model update",
        "Modello: modifica provvisoria applicata",
    ),
    (
        "snap exclusion",
        "Snap: esclusione degli elementi in movimento",
    ),
    ("plan snaps: guided", "Snap: ricerca vicino al cursore"),
    ("plan snaps: build", "Snap: indice ricostruito"),
    (
        "drawing_live: build here",
        "Geometria 2D: pianta ricostruita sul posto",
    ),
    (
        "worker: drawing started",
        "Geometria: disegni mandati in sottofondo (numero)",
    ),
    (
        "worker: scene started",
        "Scena 3D: costruzioni in sottofondo (numero)",
    ),
    ("3d rebuild", "Scena 3D: vertici ricostruiti e caricati"),
    ("3d render", "Scena 3D: comandi di disegno (CPU)"),
    ("frame: properties on", "Pannello proprietà"),
    ("show_plan", "Vista 2D completa"),
    ("frame", "Fotogramma: interfaccia (Volumen)"),
];

/// What a stretch of frames says.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub frames: usize,
    pub interactive: usize,
    pub seconds: f64,
    /// Frames a second while frames followed one another in an interaction
    /// (measured).
    pub fps: Option<f64>,
    /// Time between frames in an interaction (measured).
    pub interval: Option<Spread>,
    /// eframe's CPU time per frame in an interaction (measured): from the
    /// frame's start, when its input is read, to its rendering handed to
    /// the graphics card. This is the internal latency the app can measure.
    pub cpu: Option<Spread>,
    /// The wait for the screen per frame (estimated: interval less CPU).
    pub wait: Option<Spread>,
    /// Model changes with nothing moving.
    pub idle_updates: usize,
    /// Per section: (name, meaning, frames with it, ms per frame spread,
    /// times in all).
    pub sections: Vec<(&'static str, &'static str, usize, Option<Spread>, u32)>,
}

pub fn summarize(frames: &[Frame]) -> Summary {
    let inter: Vec<&Frame> = frames.iter().filter(|f| f.interactive()).collect();
    // intervals within an interaction: this frame and the one before both
    // at work, close together (an idle gap is not slowness)
    let mut intervals = vec![];
    let mut waits = vec![];
    for w in frames.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if let Some(i) = b.interval_ms {
            // the time between the frames not spent working on the first
            let pause = i - a.cpu_ms.unwrap_or(a.ui_ms);
            if a.interactive() && b.interactive() && pause < IDLE_GAP_MS {
                intervals.push(i);
                if let Some(c) = a.cpu_ms {
                    waits.push((i - c).max(0.0));
                }
            }
        }
    }
    let cpu: Vec<f64> = inter.iter().filter_map(|f| f.cpu_ms).collect();
    let interval = spread(&intervals);
    let sections = SECTIONS
        .iter()
        .map(|(name, what)| {
            let per: Vec<(f64, u32)> = inter.iter().filter_map(|f| f.section(name)).collect();
            let times = per.iter().map(|p| p.1).sum();
            let ms: Vec<f64> = per.iter().map(|p| p.0).collect();
            (*name, *what, per.len(), spread(&ms), times)
        })
        .collect();
    Summary {
        frames: frames.len(),
        interactive: inter.len(),
        seconds: match (frames.first(), frames.last()) {
            (Some(a), Some(b)) => b.t - a.t,
            _ => 0.0,
        },
        fps: interval.map(|s| 1000.0 / s.p50.max(0.001)),
        interval,
        cpu: spread(&cpu),
        wait: spread(&waits),
        idle_updates: frames.iter().filter(|f| f.idle_update()).count(),
        sections,
    }
}

/// Facts about the machine, gathered once off the UI thread: the
/// processor's name and, on Windows, the display's mode.
#[derive(Clone, Debug, Default)]
pub struct Machine {
    pub cpu: String,
    pub display: String,
}

static MACHINE: Mutex<Option<Machine>> = Mutex::new(None);

/// Start finding the machine's facts (on a thread; a few hundred ms on
/// Windows), if not done yet.
pub fn find_machine() {
    {
        let m = MACHINE.lock().unwrap_or_else(|e| e.into_inner());
        if m.is_some() {
            return;
        }
    }
    std::thread::spawn(|| {
        let found = Machine {
            cpu: cpu_name().unwrap_or_else(|| "non rilevato".into()),
            display: display_mode().unwrap_or_else(|| "non rilevabile".into()),
        };
        *MACHINE.lock().unwrap_or_else(|e| e.into_inner()) = Some(found);
    });
}

pub fn machine() -> Machine {
    MACHINE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or(Machine {
            cpu: "in rilevamento".into(),
            display: "in rilevamento".into(),
        })
}

#[cfg(windows)]
fn powershell(cmd: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    // no console window flashing up
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", cmd])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn cpu_name() -> Option<String> {
    #[cfg(windows)]
    {
        powershell("(Get-CimInstance Win32_Processor | Select-Object -First 1).Name")
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
    }
}

fn display_mode() -> Option<String> {
    #[cfg(windows)]
    {
        powershell(
            "Get-CimInstance Win32_VideoController | ForEach-Object { '{0} x {1}, {2} Hz' -f $_.CurrentHorizontalResolution, $_.CurrentVerticalResolution, $_.CurrentRefreshRate }",
        )
        .map(|s| s.lines().map(str::trim).collect::<Vec<_>>().join("; "))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn fmt_spread(s: Option<Spread>) -> String {
    match s {
        Some(s) => format!(
            "mediana {:.1} ms, p95 {:.1}, p99 {:.1}, massimo {:.1} ({} valori)",
            s.p50, s.p95, s.p99, s.max, s.n
        ),
        None => "nessun dato".into(),
    }
}

/// What the report says about the app and its project: no names, no
/// paths, no content.
#[derive(Clone, Debug, Default)]
pub struct Context {
    pub gpu: String,
    pub os: String,
    pub window: String,
    pub settings: Vec<(String, String)>,
    /// Elements by kind.
    pub elements: Vec<(String, usize)>,
    pub selected: usize,
    pub command: String,
}

/// The report: the context, the summary and every frame, as text to save
/// and send.
pub fn report(ctx: &Context, frames: &[Frame]) -> String {
    let m = machine();
    let s = summarize(frames);
    let mut o = String::new();
    let mut line = |t: String| {
        o.push_str(&t);
        o.push('\n');
    };
    line("Volumen - rapporto di diagnostica delle prestazioni".into());
    line(format!(
        "Versione {} · commit {} · compilata il {}",
        env!("CARGO_PKG_VERSION"),
        env!("CREABIM_COMMIT"),
        {
            let (y, m, d) = civil(env!("CREABIM_BUILD_DAY").parse().unwrap_or(0));
            format!("{d:02}/{m:02}/{y}")
        }
    ));
    line(String::new());
    line("[Sistema]".into());
    line(format!("Sistema operativo: {}", ctx.os));
    line(format!(
        "Processore: {} ({} thread)",
        m.cpu,
        std::thread::available_parallelism().map_or(0, |n| n.get())
    ));
    line(format!("Scheda grafica, backend e driver: {}", ctx.gpu));
    line(format!("Schermo (da Windows): {}", m.display));
    line(format!("Finestra di Volumen: {}", ctx.window));
    line("Presentazione: VSync con fino a 2 fotogrammi in coda (configurazione predefinita di egui-wgpu, non cambiata da Volumen; dal codice, non rilevata)".into());
    line(String::new());
    line("[Impostazioni]".into());
    for (k, v) in &ctx.settings {
        line(format!("{k}: {v}"));
    }
    line(String::new());
    line("[Progetto] (solo i conteggi, nessun nome né contenuto)".into());
    for (k, n) in &ctx.elements {
        line(format!("{k}: {n}"));
    }
    line(format!("Elementi selezionati: {}", ctx.selected));
    line(format!("Comando attivo: {}", ctx.command));
    line(String::new());
    line("[Registrazione]".into());
    line(format!(
        "Durata {:.1} s, {} fotogrammi, di cui {} durante un'interazione",
        s.seconds, s.frames, s.interactive
    ));
    line(format!(
        "FPS durante l'interazione (misurati): {}",
        s.fps.map_or(
            "nessun dato (servono fotogrammi d'interazione consecutivi, senza pause oltre 250 ms: muovere il mouse con continuità)".into(),
            |f| format!("{f:.1}")
        )
    ));
    line(format!(
        "Tempo tra fotogrammi, durante l'interazione (misurato): {}",
        fmt_spread(s.interval)
    ));
    line(format!(
        "Latenza interna, dall'inizio del fotogramma (lettura dell'input) alla consegna alla scheda grafica (misurata da eframe): {}",
        fmt_spread(s.cpu)
    ));
    line(format!(
        "Attesa dello schermo (VSync, coda) per fotogramma (stima: tempo tra fotogrammi meno CPU): {}",
        fmt_spread(s.wait)
    ));
    line("Tempo GPU: non disponibile (il motore grafico non lo fornisce a Volumen)".into());
    line("Latenza fisica, dal mouse alla luce sul monitor: non misurabile dal programma (serve una misura esterna)".into());
    line(format!(
        "Aggiornamenti del modello con il mouse fermo: {}",
        s.idle_updates
    ));
    line(String::new());
    line("[Sezioni, durante l'interazione] (i tempi comprendono le sezioni interne)".into());
    for (name, what, frames, sp, times) in &s.sections {
        line(if is_count(name) {
            format!("{what} [{name}]: in {frames} fotogrammi, {times} volte")
        } else {
            format!(
                "{what} [{name}]: in {frames} fotogrammi, {times} volte; per fotogramma {}",
                fmt_spread(*sp)
            )
        });
    }
    line(String::new());
    line("[Fotogrammi] t_s;intervallo_ms;interfaccia_ms;cpu_ms;eventi;puntatore;tasti;lavoro;modello_cambiato;sezioni".into());
    for f in frames {
        let secs: Vec<String> = f
            .sections
            .iter()
            .map(|(n, ms, k)| format!("{n}={ms:.2}x{k}"))
            .collect();
        line(format!(
            "{:.4};{};{:.2};{};{};{};{};{};{};{}",
            f.t,
            f.interval_ms.map_or("".into(), |v| format!("{v:.2}")),
            f.ui_ms,
            f.cpu_ms.map_or("".into(), |v| format!("{v:.2}")),
            f.events,
            u8::from(f.pointer_moved),
            u8::from(f.buttons_down),
            u8::from(f.working),
            u8::from(f.model_changed),
            secs.join("|")
        ));
    }
    o
}

/// The civil date (year, month, day) of a count of days since 1/1/1970
/// (Howard Hinnant's algorithm).
fn civil(day: i64) -> (i64, i64, i64) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A report's file name: when (UTC) and which build.
pub fn report_name() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (day, s) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil(day);
    format!(
        "Volumen-diagnostica-{y:04}{m:02}{d:02}-{:02}{:02}{:02}-{}.txt",
        s / 3600,
        s / 60 % 60,
        s % 60,
        env!("CREABIM_COMMIT").replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    )
}

/// The element kinds of `model`, counted (the kind's name only).
pub fn kinds(model: &creabim_core::Model) -> Vec<(String, usize)> {
    let mut by: HashMap<String, usize> = HashMap::new();
    for e in model.iter() {
        let k = format!("{:?}", e.kind);
        let k = k.split(['(', ' ', '{']).next().unwrap_or("?").to_string();
        *by.entry(k).or_insert(0) += 1;
    }
    let mut v: Vec<_> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::ONE;

    fn frame(t: f64, interval: Option<f64>, cpu: Option<f64>, events: u32, working: bool) -> Frame {
        Frame {
            t,
            interval_ms: interval,
            cpu_ms: cpu,
            events,
            working,
            ..Default::default()
        }
    }

    #[test]
    fn spread_gives_the_median_percentiles_and_maximum() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = spread(&v).unwrap();
        assert_eq!(
            (s.n, s.p50, s.p95, s.p99, s.max),
            (100, 51.0, 95.0, 99.0, 100.0)
        );
        assert!(spread(&[]).is_none());
    }

    #[test]
    fn idle_gaps_are_not_counted_as_slowness() {
        // two bursts of interaction at 16 ms, a 2 s pause between them
        let mut f = vec![];
        let mut t = 0.0;
        for i in 0..10 {
            f.push(frame(t, (i > 0).then_some(16.0), Some(5.0), 1, false));
            t += 0.016;
        }
        t += 2.0;
        f.push(frame(t, Some(2000.0), Some(5.0), 1, false));
        for _ in 0..10 {
            t += 0.016;
            f.push(frame(t, Some(16.0), Some(5.0), 1, false));
        }
        let s = summarize(&f);
        let i = s.interval.unwrap();
        assert_eq!(i.max, 16.0, "the pause is idleness, not a slow frame");
        assert!((s.fps.unwrap() - 62.5).abs() < 0.01);
        assert_eq!(
            s.wait.unwrap().p50,
            11.0,
            "the wait is estimated as interval less CPU"
        );
    }

    #[test]
    fn slow_frames_in_a_row_are_slowness_not_idleness() {
        // frames of 600 ms of work each, one after the other
        let mut f = vec![];
        for i in 0..6 {
            f.push(frame(
                f64::from(i) * 0.61,
                (i > 0).then_some(610.0),
                Some(600.0),
                1,
                true,
            ));
        }
        let s = summarize(&f);
        let i = s.interval.expect("the slow frames are measured");
        assert_eq!((i.n, i.max), (5, 610.0));
        assert!(s.fps.unwrap() < 2.0);
    }

    #[test]
    fn a_model_change_with_nothing_moving_is_an_idle_update() {
        let mut busy = frame(0.0, None, None, 0, true);
        busy.model_changed = true;
        let mut moved = frame(0.0, None, None, 1, true);
        moved.model_changed = true;
        assert!(busy.idle_update());
        assert!(!moved.idle_update());
        assert_eq!(summarize(&[busy, moved]).idle_updates, 1);
    }

    #[test]
    fn off_nothing_is_kept_and_on_frames_and_sections_are() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        set_enabled(false);
        frame_begin(1, true, false, None);
        section("snap exclusion", 3.0);
        frame_end(true, true);
        assert!(recent(60.0).is_empty(), "off: nothing kept");
        set_enabled(true);
        frame_begin(1, true, false, None);
        section("snap exclusion", 3.0);
        section("snap exclusion", 2.0);
        count("worker: drawing started");
        frame_end(true, true);
        frame_begin(0, false, false, Some(0.012));
        frame_end(false, false);
        let f = recent(60.0);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].section("snap exclusion"), Some((5.0, 2)));
        assert_eq!(f[0].section("worker: drawing started"), Some((0.0, 1)));
        let cpu = f[0]
            .cpu_ms
            .expect("eframe's CPU time goes to the frame it measured");
        assert!((cpu - 12.0).abs() < 1e-3, "{cpu}");
        set_enabled(false);
        assert!(recent(60.0).is_empty(), "off again: dropped");
    }

    #[test]
    fn a_recording_keeps_the_frames_of_its_time() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        set_enabled(true);
        start_recording();
        assert!(recording_left().unwrap() > RECORD_SECONDS - 1.0);
        for _ in 0..5 {
            frame_begin(1, true, false, Some(0.004));
            frame_end(true, false);
        }
        assert!(take_recording(false).is_none(), "not before its time");
        let r = take_recording(true).unwrap();
        assert_eq!(r.len(), 5);
        assert!(recording_left().is_none());
        set_enabled(false);
    }

    #[test]
    fn the_report_names_no_project_and_no_path() {
        let mut model = creabim_core::starter::starter_house();
        let secret = "Villa Segreta Rossi";
        let id = model.iter().next().unwrap().id;
        let mut e = model.get(id).unwrap().clone();
        e.name = secret.into();
        model = creabim_core::Model::from_elements(model.iter().cloned().map(|x| {
            if x.id == id {
                e.clone()
            } else {
                x
            }
        }))
        .unwrap();
        let ctx = Context {
            gpu: "Scheda (Dx12, driver 1)".into(),
            os: "Windows 11".into(),
            window: "3840 x 2160 pixel, scala 1.5".into(),
            settings: vec![("Stile 3D".into(), "Shaded".into())],
            elements: kinds(&model),
            selected: 3,
            command: "Move".into(),
        };
        let mut f = frame(0.0, None, Some(4.0), 1, true);
        f.sections.push(("snap exclusion", 2.0, 1));
        let text = report(&ctx, &[f.clone(), f]);
        assert!(!text.contains(secret), "no element names");
        assert!(
            !text.contains("/home") && !text.contains("C:\\"),
            "no paths"
        );
        for want in [
            "[Sistema]",
            "Scheda (Dx12, driver 1)",
            "Elementi selezionati: 3",
            "Comando attivo: Move",
            "Wall:",
            "Tempo GPU: non disponibile",
            "non misurabile dal programma",
            "Snap: esclusione degli elementi in movimento",
            "[Fotogrammi]",
        ] {
            assert!(text.contains(want), "the report says {want:?}:\n{text}");
        }
    }

    #[test]
    fn day_counts_become_calendar_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_785), (2026, 11, 28));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }
}
