use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashSet,
    env,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub flagged: bool,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub dropped: bool,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub defer: Option<String>,
    #[serde(default)]
    pub planned: Option<String>,
    #[serde(default, rename = "estimatedMinutes")]
    pub estimated_minutes: Option<u32>,
}

#[derive(Default, Debug)]
pub struct Snapshot {
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
}

impl Snapshot {
    pub fn from_json(projects: Value, tasks: Value) -> Result<Self, String> {
        let projects: Vec<Project> = serde_json::from_value(projects).map_err(|e| e.to_string())?;
        let tasks: Vec<Task> = serde_json::from_value(tasks).map_err(|e| e.to_string())?;
        let roots: HashSet<&str> = projects.iter().map(|p| p.id.as_str()).collect();
        // of returns project root tasks in task list; exposing these could complete a project.
        let tasks = tasks
            .into_iter()
            .filter(|t| !roots.contains(t.id.as_str()) && !t.completed && !t.dropped)
            .collect();
        let projects = projects
            .into_iter()
            .filter(|p| p.status != "done" && p.status != "dropped")
            .collect();
        Ok(Self { projects, tasks })
    }
}

#[derive(Clone)]
pub struct Backend {
    executable: PathBuf,
}

impl Backend {
    pub fn new() -> Self {
        let executable = env::var_os("OMNIFOCUS_CLI")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if let Some(path) = env::var_os("PATH").and_then(|path| {
                    env::split_paths(&path)
                        .map(|p| p.join("of"))
                        .find(|p| p.is_file())
                }) {
                    return path;
                }
                PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".bun/bin/of")
            });
        Self { executable }
    }

    pub fn call(&self, args: &[&str]) -> Result<Value, String> {
        let mut child = Command::new(&self.executable)
            .arg("--compact")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                format!("无法运行 of：{e}。请安装 bun install -g @stephendolan/omnifocus-cli")
            })?;
        let stdout = child.stdout.take().ok_or("无法读取 of stdout")?;
        let stderr = child.stderr.take().ok_or("无法读取 of stderr")?;
        let read = |mut stream: Box<dyn Read + Send>| {
            thread::spawn(move || {
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).map(|_| bytes)
            })
        };
        let out = read(Box::new(stdout));
        let err = read(Box::new(stderr));
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if start.elapsed() < Duration::from_secs(40) => {
                    thread::sleep(Duration::from_millis(25));
                }
                result => {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Readers own their pipes and exit when the process tree closes them.
                    return Err(match result {
                        Err(e) => format!("of 执行失败：{e}"),
                        _ => "OmniFocus 响应超时；写入可能已生效，请刷新核对后再重试。".into(),
                    });
                }
            }
        };
        let stdout = out
            .join()
            .map_err(|_| "of stdout 线程失败")?
            .map_err(|e| e.to_string())?;
        let stderr = err
            .join()
            .map_err(|_| "of stderr 线程失败")?
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!(
                "of 执行失败：{}",
                String::from_utf8_lossy(if stderr.is_empty() { &stdout } else { &stderr }).trim()
            ));
        }
        let value: Value =
            serde_json::from_slice(&stdout).map_err(|e| format!("of 返回了无效 JSON：{e}"))?;
        if let Some(error) = value.get("error") {
            return Err(error.to_string());
        }
        Ok(value)
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        // Include closed projects so their root tasks can never become actionable rows.
        let projects = self.call(&["project", "list", "--dropped"])?;
        let tasks = self.call(&["task", "list"])?;
        Snapshot::from_json(projects, tasks)
    }

    pub fn capture(&self, name: &str) -> Result<Value, String> {
        if name.trim().is_empty() {
            return Err("任务名称不能为空".into());
        }
        self.call(&["inbox", "add", "--", name.trim()])
    }

    pub fn create(&self, name: &str, note: &str, project: Option<&str>) -> Result<Value, String> {
        let mut args: Vec<String> = vec!["task".into(), "create".into()];
        if let Some(project) = project {
            args.push("--project".into());
            args.push(project.into());
        }
        if !note.is_empty() {
            // 用 = 形式传值，避免备注以 - 开头时被当作选项。
            args.push(format!("--note={note}"));
        }
        args.push("--".into());
        args.push(name.trim().into());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.call(&args)
    }

    pub fn complete(&self, id: &str) -> Result<Value, String> {
        self.call(&["task", "update", id, "--complete"])
    }

    pub fn drop(&self, id: &str) -> Result<Value, String> {
        self.call(&["task", "update", id, "--drop"])
    }

    pub fn update(&self, id: &str, name: &str, note: &str) -> Result<Value, String> {
        // 用 = 形式传值，避免名称或备注以 - 开头时被当作选项。
        self.call(&[
            "task",
            "update",
            id,
            &format!("--name={name}"),
            &format!("--note={note}"),
        ])
    }

    pub fn flag(&self, id: &str, flagged: bool) -> Result<Value, String> {
        self.call(&[
            "task",
            "update",
            id,
            if flagged { "--flag" } else { "--unflag" },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn excludes_project_roots_closed_projects_and_finished_tasks() {
        let snapshot = Snapshot::from_json(
            json!([{"id":"p1","name":"工作","status":"active"}, {"id":"p2","name":"旧项目","status":"done"}]),
            json!([
                {"id":"p1","name":"工作"}, {"id":"p2","name":"旧项目"},
                {"id":"t1","name":"真正任务","project":"工作"},
                {"id":"t2","name":"已完成","completed":true},
                {"id":"t3","name":"已放弃","dropped":true}
            ]),
        ).unwrap();
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].id, "t1");
    }

    #[test]
    fn refuses_invalid_payloads() {
        assert!(Snapshot::from_json(json!({"error":"Permission denied"}), json!([])).is_err());
        assert!(Snapshot::from_json(json!([]), json!([{"name":"missing id"}])).is_err());
    }
}
