use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use rust_mcp_macros::JsonSchema;
use rust_mcp_schema::{Task, TaskStatus};
use rust_mcp_sdk::{
    task_store::{ClientTaskStore, ServerTaskStore, TaskStore},
    SessionId,
};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};
const MIN_TIME: u64 = 15;

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TaskJobInfo {
    pub finish_in_ms: u64,
    pub status_interval_ms: u64,
    pub task_final_status: String,
    pub task_result: Option<String>,
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
}

pub struct McpTaskRunner {
    tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl McpTaskRunner {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub async fn run_server_task(
        &self,
        task: Task,
        task_store: Arc<ServerTaskStore>,
        job_info: TaskJobInfo,
        session_id: Option<SessionId>,
    ) -> Task {
        let task_id = task.task_id.to_string();
        let task_id_clone = task_id.clone();

        let tasks = Arc::clone(&self.tasks);

        let handle = tokio::spawn(async move {
            let start = Instant::now();
            let mut next_status = Duration::from_millis(job_info.status_interval_ms.max(MIN_TIME));

            let total_duration = Duration::from_millis(job_info.finish_in_ms);

            let final_status =
                serde_json::from_str::<TaskStatus>(&format!("\"{}\"", job_info.task_final_status))
                    .unwrap();

            loop {
                let elapsed = start.elapsed();

                // final completion
                if elapsed >= total_duration {
                    match job_info.task_result {
                        Some(result) => {
                            task_store
                                .store_task_result(
                                    &task_id_clone,
                                    final_status,
                                    serde_json::from_str(&result).unwrap(),
                                    session_id.clone(),
                                )
                                .await
                        }
                        None => {
                            task_store
                                .update_task_status(
                                    &task_id_clone,
                                    final_status,
                                    Some(format!(
                                        "task status updated to {}",
                                        job_info.task_final_status
                                    )),
                                    session_id.clone(),
                                )
                                .await
                        }
                    };

                    break;
                }

                // periodic status update
                if elapsed >= next_status {
                    task_store
                        .update_task_status(
                            &task_id_clone,
                            TaskStatus::Working,
                            Some(format!(
                                "time elapsed {} , still working...",
                                elapsed.as_millis()
                            )),
                            session_id.clone(),
                        )
                        .await;
                    next_status += Duration::from_millis(job_info.status_interval_ms);
                }

                // sleep a bit to avoid busy looping
                sleep(Duration::from_millis(MIN_TIME)).await;
            }

            let mut tasks = tasks.lock().await;
            tasks.remove(&task_id_clone);
        });

        self.tasks.lock().await.insert(task_id, handle);

        task
    }

    pub async fn run_client_task(
        &self,
        task: Task,
        task_store: Arc<ClientTaskStore>,
        job_info: TaskJobInfo,
        session_id: Option<SessionId>,
    ) -> Task {
        let task_id = task.task_id.to_string();
        let task_id_clone = task_id.clone();

        let tasks = Arc::clone(&self.tasks);

        let handle = tokio::spawn(async move {
            let start = Instant::now();
            let mut next_status = Duration::from_millis(job_info.status_interval_ms.max(MIN_TIME));

            let total_duration = Duration::from_millis(job_info.finish_in_ms);

            let final_status =
                serde_json::from_str::<TaskStatus>(&format!("\"{}\"", job_info.task_final_status))
                    .unwrap();

            loop {
                let elapsed = start.elapsed();

                // final completion
                if elapsed >= total_duration {
                    match job_info.task_result {
                        Some(result) => {
                            task_store
                                .store_task_result(
                                    &task_id_clone,
                                    final_status,
                                    serde_json::from_str(&result).unwrap(),
                                    session_id.clone(),
                                )
                                .await
                        }
                        None => {
                            task_store
                                .update_task_status(
                                    &task_id_clone,
                                    final_status,
                                    Some(format!(
                                        "task status updated to {}",
                                        job_info.task_final_status
                                    )),
                                    session_id.clone(),
                                )
                                .await
                        }
                    };

                    break;
                }

                // periodic status update
                if elapsed >= next_status {
                    task_store
                        .update_task_status(
                            &task_id_clone,
                            TaskStatus::Working,
                            Some(format!(
                                "time elapsed {} , still working...",
                                elapsed.as_millis()
                            )),
                            session_id.clone(),
                        )
                        .await;
                    next_status += Duration::from_millis(job_info.status_interval_ms);
                }

                // sleep a bit to avoid busy looping
                sleep(Duration::from_millis(MIN_TIME)).await;
            }

            let mut tasks = tasks.lock().await;
            tasks.remove(&task_id_clone);
        });

        self.tasks.lock().await.insert(task_id, handle);

        task
    }
}
