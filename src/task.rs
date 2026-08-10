//! 后台任务框架：工作线程执行耗时操作，通过通道向 UI 汇报进度。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::archive::OpenArchive;

/// 任务完成后可能携带的产物。
pub enum JobOutcome {
    None,
    /// 压缩包已打开（可能经过分卷合并）。
    Opened(Box<OpenArchive>),
    /// 临时解出的单个文件，用于“查看”。
    Previewed(std::path::PathBuf, String),
}

pub enum JobMsg {
    Total(u64),
    Progress { done: u64, label: String },
    Log(String),
    Finished(Result<(String, JobOutcome), String>),
}

/// 传给工作线程的进度汇报器。
#[derive(Clone)]
pub struct Reporter {
    tx: Sender<JobMsg>,
    ctx: egui::Context,
    cancel: Arc<AtomicBool>,
}

impl Reporter {
    /// 构造一个不绑定 UI 的汇报器，供测试或无界面场景使用。
    #[cfg(test)]
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, _rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        Reporter {
            tx,
            ctx,
            cancel,
        }
    }

    pub fn total(&self, total: u64) {
        let _ = self.tx.send(JobMsg::Total(total));
        self.ctx.request_repaint();
    }

    pub fn progress(&self, done: u64, label: &str) {
        let _ = self.tx.send(JobMsg::Progress {
            done,
            label: label.to_string(),
        });
        self.ctx.request_repaint();
    }

    pub fn log(&self, line: impl Into<String>) {
        let _ = self.tx.send(JobMsg::Log(line.into()));
        self.ctx.request_repaint();
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// 在循环里检查取消标记，取消时返回错误以中断任务。
    pub fn check_cancel(&self) -> anyhow::Result<()> {
        if self.cancelled() {
            anyhow::bail!("操作已被用户取消");
        }
        Ok(())
    }
}

/// UI 侧持有的运行中任务状态。
pub struct RunningJob {
    pub title: String,
    rx: Receiver<JobMsg>,
    cancel: Arc<AtomicBool>,
    pub total: u64,
    pub done: u64,
    pub label: String,
    pub logs: Vec<String>,
    pub finished: Option<Result<(String, JobOutcome), String>>,
}

impl RunningJob {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelling(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// 每帧调用，抽干通道里的消息。返回 true 表示任务已结束。
    pub fn poll(&mut self) -> bool {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                JobMsg::Total(t) => self.total = t,
                JobMsg::Progress { done, label } => {
                    self.done = done;
                    self.label = label;
                }
                JobMsg::Log(line) => {
                    if self.logs.len() > 500 {
                        self.logs.remove(0);
                    }
                    self.logs.push(line);
                }
                JobMsg::Finished(res) => {
                    self.finished = Some(res);
                }
            }
        }
        self.finished.is_some()
    }

    pub fn take_result(&mut self) -> Option<Result<(String, JobOutcome), String>> {
        self.finished.take()
    }
}

/// 启动一个后台任务。
pub fn spawn<F>(ctx: &egui::Context, title: impl Into<String>, work: F) -> RunningJob
where
    F: FnOnce(&Reporter) -> anyhow::Result<(String, JobOutcome)> + Send + 'static,
{
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let reporter = Reporter {
        tx: tx.clone(),
        ctx: ctx.clone(),
        cancel: cancel.clone(),
    };
    let ctx2 = ctx.clone();

    std::thread::spawn(move || {
        let result = work(&reporter).map_err(|e| format!("{e:#}"));
        let _ = tx.send(JobMsg::Finished(result));
        ctx2.request_repaint();
    });

    RunningJob {
        title: title.into(),
        rx,
        cancel,
        total: 0,
        done: 0,
        label: String::new(),
        logs: Vec::new(),
        finished: None,
    }
}
