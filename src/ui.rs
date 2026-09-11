use console::{style, Term};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StreamStep {
    pub step_num: usize,
    pub total_steps: usize,
    pub description: String,
    pub status: StepStatus,
    pub start_time: Option<Instant>,
    pub elapsed: f64,
    pub detail: String,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

struct EngineState {
    action: String,
    target: String,
    total_steps: usize,
    steps: Vec<StreamStep>,
    start_time: Instant,
    is_failed: bool,
    is_finished: bool,
    rendered_lines: usize,
}

#[derive(Clone)]
pub struct StreamEngine {
    state: Arc<Mutex<EngineState>>,
    running: Arc<AtomicBool>,
}

impl StreamEngine {
    pub fn new(action: &str, target: &str, total_steps: usize) -> Self {
        let state = Arc::new(Mutex::new(EngineState {
            action: action.to_string(),
            target: target.to_string(),
            total_steps,
            steps: Vec::new(),
            start_time: Instant::now(),
            is_failed: false,
            is_finished: false,
            rendered_lines: 0,
        }));
        let running = Arc::new(AtomicBool::new(false));

        Self { state, running }
    }

    pub fn add_step(&self, description: &str) -> usize {
        let mut state = self.state.lock().unwrap();
        let idx = state.steps.len();
        let total_steps = state.total_steps;
        state.steps.push(StreamStep {
            step_num: idx + 1,
            total_steps,
            description: description.to_string(),
            status: StepStatus::Pending,
            start_time: None,
            elapsed: 0.0,
            detail: String::new(),
            logs: Vec::new(),
            error: None,
        });
        idx
    }

    pub fn start(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }

        let term = Term::stdout();
        let _ = term.hide_cursor();

        let self_clone = self.clone();
        thread::spawn(move || {
            while self_clone.running.load(Ordering::SeqCst) {
                self_clone.render();
                thread::sleep(Duration::from_millis(80)); // 12 FPS
            }
        });
    }

    pub fn start_step(&self, idx: usize, detail: &str) {
        {
            let mut state = self.state.lock().unwrap();
            if let Some(step) = state.steps.get_mut(idx) {
                step.status = StepStatus::Running;
                step.start_time = Some(Instant::now());
                step.detail = detail.to_string();
                if !detail.is_empty() {
                    step.logs.push(detail.to_string());
                }
            }
        }
        self.render();
    }

    pub fn update_step_detail(&self, idx: usize, detail: &str) {
        {
            let mut state = self.state.lock().unwrap();
            if let Some(step) = state.steps.get_mut(idx) {
                step.detail = detail.to_string();
                if !detail.is_empty()
                    && (step.logs.is_empty()
                        || step.logs.last().map(|s| s.as_str()) != Some(detail))
                {
                    step.logs.push(detail.to_string());
                    if step.logs.len() > 6 {
                        step.logs.remove(0);
                    }
                }
            }
        }
        self.render();
    }

    pub fn finish_step(&self, idx: usize, detail: &str) {
        {
            let mut state = self.state.lock().unwrap();
            if let Some(step) = state.steps.get_mut(idx) {
                step.status = StepStatus::Done;
                if let Some(st) = step.start_time {
                    step.elapsed = st.elapsed().as_secs_f64();
                }
                if !detail.is_empty() {
                    step.detail = detail.to_string();
                }
                step.logs.clear();
            }
        }
        self.render();
    }

    pub fn fail_step(&self, idx: usize, error_text: &str) {
        {
            let mut state = self.state.lock().unwrap();
            state.is_failed = true;
            if let Some(step) = state.steps.get_mut(idx) {
                step.status = StepStatus::Failed;
                if let Some(st) = step.start_time {
                    step.elapsed = st.elapsed().as_secs_f64();
                }
                step.error = Some(error_text.to_string());
            }
        }
        self.render();
    }

    pub fn stop(&self, summary_line: Option<&str>) {
        self.running.store(false, Ordering::SeqCst);
        {
            let mut state = self.state.lock().unwrap();
            state.is_finished = true;
        }
        self.render();

        let term = Term::stdout();
        let _ = term.show_cursor();

        if let Some(summary) = summary_line {
            println!("{}\n", style(format!("[+] {}", summary)).green().bold());
        } else {
            println!();
        }
    }

    fn render(&self) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        let total_elapsed = state.start_time.elapsed().as_secs_f64();
        let done_count = state
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Done)
            .count();
        let total_steps = state.total_steps;

        let spinner_frames = if cfg!(windows) {
            vec!["-", "\\", "|", "/"]
        } else {
            vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
        };
        let frame_idx = ((total_elapsed * 12.0) as usize) % spinner_frames.len();
        let spinner_char = spinner_frames[frame_idx];

        let mut lines = Vec::new();

        // 1. Header line (Docker BuildKit Style)
        let header = if state.is_failed {
            format!(
                "{} {} {} {:.1}s ({}/{}) FAILED",
                style("[-]").red().bold(),
                style(&state.action).red().bold(),
                style(&state.target).red().bold(),
                total_elapsed,
                done_count,
                total_steps
            )
        } else if state.is_finished {
            format!(
                "{} {} {} {:.1}s ({}/{}) FINISHED",
                style("[+]").cyan().bold(),
                style(&state.action).cyan().bold(),
                style(&state.target).cyan().bold(),
                total_elapsed,
                total_steps,
                total_steps
            )
        } else {
            format!(
                "{} {} {} {:.1}s ({}/{})",
                style(spinner_char).cyan().bold(),
                style(&state.action).cyan().bold(),
                style(&state.target).cyan().bold(),
                total_elapsed,
                done_count,
                total_steps
            )
        };
        lines.push(header);

        // 2. Step lines
        for step in &state.steps {
            match step.status {
                StepStatus::Pending => {
                    let prefix = style(format!(" => [{}/{}]", step.step_num, total_steps)).dim();
                    let desc = style(&step.description).dim();
                    let timer = style("...").dim();
                    lines.push(format!(" {} {:<52} {}", prefix, desc, timer));
                }
                StepStatus::Running => {
                    let prefix = style(format!(" => [{}/{}]", step.step_num, total_steps))
                        .cyan()
                        .bold();
                    let desc = style(&step.description).bold();
                    let step_elapsed = step
                        .start_time
                        .map(|t| t.elapsed().as_secs_f64())
                        .unwrap_or(0.0);
                    let timer = style(format!("{:.1}s", step_elapsed)).cyan();
                    lines.push(format!(" {} {:<52} {}", prefix, desc, timer));

                    let active_logs = if !step.logs.is_empty() {
                        &step.logs[step.logs.len().saturating_sub(3)..]
                    } else if !step.detail.is_empty() {
                        std::slice::from_ref(&step.detail)
                    } else {
                        &[]
                    };

                    for log in active_logs {
                        let sub_prefix = style(" => =>  ").cyan().bold();
                        let sub_log = style(log).cyan().dim();
                        let sub_timer = style(format!("{:.1}s", step_elapsed)).dim();
                        lines.push(format!(" {} {:<52} {}", sub_prefix, sub_log, sub_timer));
                    }
                }
                StepStatus::Done => {
                    let prefix = style(format!(" => [{}/{}]", step.step_num, total_steps))
                        .cyan()
                        .bold();
                    let desc = style(&step.description).bold();
                    let timer = style(format!("{:.1}s", step.elapsed)).dim();
                    lines.push(format!(" {} {:<52} {}", prefix, desc, timer));

                    if !step.detail.is_empty() {
                        let sub_prefix = style(" => =>  ").cyan().bold();
                        let sub_log = style(&step.detail).cyan().dim();
                        let sub_timer = style(format!("{:.1}s", step.elapsed)).dim();
                        lines.push(format!(" {} {:<52} {}", sub_prefix, sub_log, sub_timer));
                    }
                }
                StepStatus::Failed => {
                    let prefix = style(format!(" => [{}/{}]", step.step_num, total_steps))
                        .red()
                        .bold();
                    let desc = style(&step.description).red().bold();
                    let timer = style(format!("{:.1}s", step.elapsed)).red();
                    lines.push(format!(" {} {:<52} {}", prefix, desc, timer));

                    if let Some(ref err) = step.error {
                        let sub_prefix = style(" => =>  ").red().bold();
                        let sub_log = style(format!("ERROR: {}", err)).red().bold();
                        let sub_timer = style(format!("{:.1}s", step.elapsed)).dim();
                        lines.push(format!(" {} {:<52} {}", sub_prefix, sub_log, sub_timer));
                    }
                }
            }
        }

        // Atomic multi-line terminal write
        let mut buffer = String::new();
        if state.rendered_lines > 0 {
            // Move cursor up by rendered_lines lines to start of line
            buffer.push_str(&format!("\x1b[{}F", state.rendered_lines));
        }

        for line in &lines {
            // \x1b[2K clears entire line, \r carriage return
            buffer.push_str(&format!("\x1b[2K{}\r\n", line));
        }

        let mut stdout = io::stdout();
        let _ = stdout.write_all(buffer.as_bytes());
        let _ = stdout.flush();

        state.rendered_lines = lines.len();
    }
}
