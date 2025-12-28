use super::{CreateTaskOptions, TaskStore};
use crate::error::SdkResult;
use crate::task_store::TaskStatusSignal;
use crate::utils::{current_utc_time, iso8601_time};
use crate::{id_generator::FastIdGenerator, IdGenerator};
use async_trait::async_trait;
use futures::{future::BoxFuture, stream, Stream};
use rust_mcp_schema::{
    ListTasksResult, RequestId, RpcError, Task, TaskStatus, TaskStatusNotificationParams,
};
use rust_mcp_transport::{SessionId, TaskId};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::fmt::{Debug, Display};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot::Receiver;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;

/// Parameters returned by a task status polling callback.
///
/// Contains the latest known status of a task and its recommended poll interval
/// (in milliseconds). The poll interval can be adjusted dynamically by the remote
/// side to implement adaptive polling (e.g., longer intervals when idle).
pub type TaskStatusUpdate = (TaskStatus, Option<i64>);
// TaskStatusPoller
pub type GetTaskCallback = Box<
    dyn Fn(TaskId, Option<SessionId>) -> BoxFuture<'static, SdkResult<TaskStatusUpdate>>
        + Send
        + Sync
        + 'static,
>;

/// Represents a single scheduled polling operation for a task.
/// The fields are ordered intentionally for correct priority queue behavior:
/// - `next_poll_at`: The exact `Instant` when this task should be polled next.
///   The `Reverse` wrapper ensures the earliest (smallest) `Instant` is popped first (min-heap).
/// - `task_id`: Identifier of the task to poll.
/// - `session_id`: Optional session context. `None` means the task is global (not bound to any session).
///
type ScheduledPoll = (Instant, TaskId, Option<SessionId>);

const DEFAULT_PAGE_SIZE: usize = 50;
const DEFAULT_POLL_INTERVAL: i64 = 1250;

pub struct InMemoryTaskStore<Req, Res>
where
    Req: Clone + Send + Sync + 'static,
    Res: Clone + Send + Sync + 'static,
{
    id_gen: Arc<FastIdGenerator>,
    // Inner state protected by RwLock for concurrent access
    inner: Arc<tokio::sync::RwLock<InMemoryTaskStoreInner<Req, Res>>>,
    page_size: usize,
    broadcast: tokio::sync::broadcast::Sender<(TaskStatusNotificationParams, Option<String>)>,
    polling_task_handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug)]
struct TaskEntry<Req, Res> {
    task: Task,
    #[allow(unused)]
    request: Req, // original request that created the task
    result: Option<Res>, // stored only after store_task_result
    #[allow(unused)]
    expires_at: Option<i64>, // Unix millis, for reference (optional now)
    meta: Option<serde_json::Map<String, serde_json::Value>>,
    result_tx: Option<tokio::sync::oneshot::Sender<(TaskStatus, Option<Res>)>>,
}

impl<Req, Res> Display for TaskEntry<Req, Res> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "task_id: {}", self.task.task_id)?;
        writeln!(f, "created_at: {}", self.task.created_at)?;
        writeln!(f, "status: {}", self.task.status)?;
        writeln!(f, "last_updated_at: {}", self.task.last_updated_at)?;
        if let Some(message) = self.task.status_message.as_ref() {
            writeln!(f, "status_message: {}", message)?;
        }

        if let Some(ttl) = self.task.ttl.as_ref() {
            writeln!(f, "ttl: {}", ttl)?;
        } else {
            writeln!(f, "ttl: null")?;
        }
        Ok(())
    }
}

struct InMemoryTaskStoreInner<Req, Res> {
    // Map: session_id (None for global) => task_id => TaskEntry
    tasks: HashMap<Option<String>, BTreeMap<String, TaskEntry<Req, Res>>>,
    // For simple reverse-chronological pagination (newest first)
    // session_id => Vec<task_id> sorted by created_at descending
    ordered_task_ids: HashMap<Option<String>, Vec<String>>,
    // A min-heap for scheduling task-status polling when this task store is used
    // to hold requester tasks while waiting for the other party to complete them.
    pub poll_schedule: Option<BinaryHeap<Reverse<ScheduledPoll>>>, // min-heap by (next_poll_time, poll_interval,...)
}

impl<Req, Res> InMemoryTaskStoreInner<Req, Res> {
    pub(crate) fn re_schedule(&mut self, tasks: &mut Vec<(TaskId, Option<SessionId>, i64)>) {
        let Some(poll_schedule) = self.poll_schedule.as_mut() else {
            return;
        };

        let now = Instant::now();
        let to_reschedule = tasks.drain(0..);

        for (task_id, session_id, poll_interval) in to_reschedule {
            let next_poll = now
                .checked_add(Duration::from_millis(poll_interval as u64))
                .unwrap_or(Instant::now());
            poll_schedule.push(Reverse((next_poll, task_id, session_id)));
        }
    }

    pub(crate) fn get_task(
        &self,
        task_id: &str,
        session_id: &Option<String>,
    ) -> Option<&TaskEntry<Req, Res>> {
        self.tasks
            .get(session_id)
            .and_then(|session_map| session_map.get(task_id))
    }

    pub(crate) fn remove_task(
        &mut self,
        task_id: &str,
        session_id: &Option<String>,
    ) -> Option<TaskEntry<Req, Res>> {
        self.tasks
            .get_mut(session_id)
            .and_then(|session_map| session_map.remove(task_id))
    }

    pub(crate) fn next_sleep_duration(&self) -> Duration {
        let now = Instant::now();

        if let Some(poll_schedule) = self.poll_schedule.as_ref() {
            if let Some(Reverse(entry)) = poll_schedule.peek() {
                return entry.0.duration_since(now);
            }
        };

        Duration::from_millis(DEFAULT_POLL_INTERVAL as u64)
    }

    pub(crate) fn tasks_to_poll(&mut self) -> Vec<(TaskId, Option<SessionId>)> {
        let now = Instant::now();

        let Some(poll_schedule) = self.poll_schedule.as_mut() else {
            return vec![];
        };

        let mut task_ids = Vec::new();

        while let Some(Reverse(entry)) = poll_schedule.peek() {
            let (next_poll, task_id, session_id) = &entry;

            if next_poll <= &now {
                // Pop the task from the schedule
                task_ids.push((task_id.clone(), session_id.clone()));
                poll_schedule.pop();

            // Add task id to the list
            } else {
                break; // Stop once the task's next_poll > now
            }
        }

        task_ids
    }

    async fn subscribe_to_task(
        &mut self,
        task_id: &str,
        session_id: &Option<String>,
    ) -> Option<Receiver<(TaskStatus, Option<Res>)>> {
        let Some(entry) = self
            .tasks
            .get_mut(session_id)
            .and_then(|session_map| session_map.get_mut(task_id))
        else {
            return None;
        };

        let (tx_response, rx_response) = oneshot::channel::<(TaskStatus, Option<Res>)>();
        entry.result_tx = Some(tx_response);
        Some(rx_response)
    }
}

impl<Req, Res> InMemoryTaskStore<Req, Res>
where
    Req: Debug + Clone + Send + Sync + serde::Deserialize<'static> + serde::Serialize + 'static,
    Res: Debug + Clone + Send + Sync + serde::Deserialize<'static> + serde::Serialize + 'static,
{
    pub fn new(page_size: Option<usize>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(InMemoryTaskStoreInner {
                tasks: HashMap::new(),
                ordered_task_ids: HashMap::new(),
                poll_schedule: Some(BinaryHeap::new()),
            })),
            broadcast: tokio::sync::broadcast::channel(64).0,
            page_size: page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            id_gen: Arc::new(FastIdGenerator::new(Some("tsk"))),
            polling_task_handle: Mutex::new(None),
        }
    }
}

impl<Req, Res> InMemoryTaskStore<Req, Res>
where
    Req: Debug + Clone + Send + Sync + serde::Deserialize<'static> + serde::Serialize + 'static,
    Res: Debug + Clone + Send + Sync + serde::Deserialize<'static> + serde::Serialize + 'static,
{
    async fn notify_status_change(
        &self,
        task_entry: &TaskEntry<Req, Res>,
        session_id: Option<&String>,
    ) {
        let task = &task_entry.task;
        let params = TaskStatusNotificationParams {
            created_at: task.created_at.to_owned(),
            last_updated_at: task.last_updated_at.to_owned(),
            meta: task_entry.meta.clone(),
            poll_interval: task.poll_interval,
            status: task.status,
            status_message: task.status_message.clone(),
            task_id: task.task_id.clone(),
            ttl: task.ttl,
        };
        self.publish_status_change(params, session_id).await;
    }
}

#[async_trait]
impl<Req, Res> TaskStatusSignal for InMemoryTaskStore<Req, Res>
where
    Req: Clone + Debug + Send + Sync + 'static + serde::Deserialize<'static> + serde::Serialize,
    Res: Clone + Debug + Send + Sync + 'static + serde::Deserialize<'static> + serde::Serialize,
{
    async fn publish_status_change(
        &self,
        event: TaskStatusNotificationParams,
        session_id: Option<&String>,
    ) {
        let _ = self.broadcast.send((event, session_id.cloned()));
    }

    fn subscribe(
        &self,
    ) -> Option<
        Pin<
            Box<dyn Stream<Item = (TaskStatusNotificationParams, Option<String>)> + Send + 'static>,
        >,
    > {
        let rx = self.broadcast.subscribe();
        let stream = stream::unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(item) => return Some((item, rx)),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!("Broadcast lagged: skipped {} messages", skipped);
                        continue;
                    }
                }
            }
        });

        Some(Box::pin(stream))
    }
}

#[async_trait]
impl<Req, Res> TaskStore<Req, Res> for InMemoryTaskStore<Req, Res>
where
    Req: Clone + Debug + Send + Sync + 'static + serde::Deserialize<'static> + serde::Serialize,
    Res: Clone + Debug + Send + Sync + 'static + serde::Deserialize<'static> + serde::Serialize,
{
    async fn create_task(
        &self,
        task_params: CreateTaskOptions,
        _request_id: RequestId,
        request: Req,
        session_id: Option<String>,
    ) -> Task {
        let mut inner = self.inner.write().await;
        let task_id: String = self.id_gen.generate();
        let created_at = iso8601_time(current_utc_time(None));
        let task = Task {
            task_id: task_id.clone(),
            created_at: created_at.clone(),
            status: TaskStatus::Working,
            poll_interval: task_params.poll_interval,
            ttl: task_params.ttl,
            status_message: None,
            last_updated_at: created_at.clone(),
        };

        let entry = TaskEntry {
            task: task.clone(),
            request,
            result: None,
            expires_at: task_params
                .ttl
                .map(|ttl| current_utc_time(Some(ttl)).unix_timestamp()),
            meta: task_params.meta,
            result_tx: None,
        };

        // schedule the tasl for polling
        if let Some(schedule) = inner.poll_schedule.as_mut() {
            let poll_interval: i64 = task_params.poll_interval.unwrap_or(DEFAULT_POLL_INTERVAL);
            let next_poll = Instant::now()
                .checked_add(Duration::from_millis(poll_interval as u64))
                .unwrap_or(Instant::now());

            schedule.push(Reverse((next_poll, task_id.clone(), session_id.clone())));
        }

        tracing::debug!(
            "New task created: {entry} \n{}",
            session_id
                .as_ref()
                .map_or(String::new(), |s| format!("Session: {s}"))
        );

        // Insert into tasks map
        let session_tasks = inner
            .tasks
            .entry(session_id.clone())
            .or_insert_with(BTreeMap::new);
        session_tasks.insert(task_id.clone(), entry);

        // Insert into ordered list (newest first)
        let ordered = inner
            .ordered_task_ids
            .entry(session_id.clone())
            .or_insert_with(Vec::new);
        ordered.insert(0, task_id.clone()); // newest at front

        // Handle TTL: spawn a one-time cleanup task if ttl is set
        if let Some(ttl_duration) = task_params.ttl {
            let inner_clone = self.inner.clone();
            let session_id_clone = session_id.clone();
            let task_id_clone = task_id.clone();

            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(ttl_duration as u64)).await;

                let mut write_guard = inner_clone.write().await;

                // Remove from tasks map
                if let Some(session_map) = write_guard.tasks.get_mut(&session_id_clone) {
                    session_map.remove(&task_id_clone);
                }

                // Remove from ordered list
                if let Some(ordered_ids) = write_guard.ordered_task_ids.get_mut(&session_id_clone) {
                    if let Some(pos) = ordered_ids.iter().position(|id| id == &task_id_clone) {
                        ordered_ids.remove(pos);
                    }
                }

                // Optional: clean up empty session entries
                write_guard.tasks.retain(|_, map| !map.is_empty());
                write_guard
                    .ordered_task_ids
                    .retain(|_, vec| !vec.is_empty());

                tracing::debug!("Task {} expired and removed due to TTL", task_id_clone);
            });
        }

        task
    }

    fn start_task_polling(&self, get_task_callback: GetTaskCallback) -> SdkResult<()> {
        match self
            .polling_task_handle
            .lock()
            .and_then(|v| Ok(v.is_some()))
        {
            Ok(has_value) if has_value => {
                return Err(RpcError::internal_error()
                    .with_message("Task polling is already running.")
                    .into())
            }
            Err(err) => {
                return Err(RpcError::internal_error()
                    .with_message(err.to_string())
                    .into())
            }
            _ => {}
        }

        let inner = self.inner.clone();
        let handle = tokio::spawn(async move {
            loop {
                let mut to_reschedule: Vec<(TaskId, Option<SessionId>, i64)> = Vec::new();
                let tasks_to_poll = {
                    let mut guard = inner.write().await;
                    guard.tasks_to_poll()
                };
                for (task_id, session_id) in tasks_to_poll {
                    // TODO: avoid cloning
                    match get_task_callback(task_id.clone(), session_id.clone()).await {
                        Ok((task_status, poll_interval)) => {
                            if *&task_status.is_terminal() {
                                // remove the task and resolve awaiting task if in terminal state
                                let mut guard = inner.write().await;
                                let entry = guard.remove_task(&task_id, &session_id);
                                if let Some(task_entry) = entry {
                                    if let Some(result_tx) = task_entry.result_tx {
                                        result_tx.send((task_status, task_entry.result));
                                    }
                                }
                            } else {
                                to_reschedule.push((
                                    task_id.clone(),
                                    session_id,
                                    poll_interval.unwrap_or(DEFAULT_POLL_INTERVAL),
                                ));
                            }
                        }
                        Err(_err) => {
                            let guard = inner.read().await;
                            // re-schedule if task still exists and not expired
                            if let Some(get_task) = guard.get_task(&task_id, &session_id) {
                                to_reschedule.push((
                                    task_id,
                                    session_id,
                                    get_task.task.poll_interval.unwrap_or(DEFAULT_POLL_INTERVAL),
                                ));
                            }
                        }
                    }
                }

                if !to_reschedule.is_empty() {
                    let mut guard = inner.write().await;
                    guard.re_schedule(&mut to_reschedule)
                }

                let guard = inner.read().await;
                let sleep_duration = guard.next_sleep_duration();

                tokio::time::sleep(sleep_duration).await;
            }
        });

        let mut lock = match self.polling_task_handle.lock() {
            Ok(value) => value,
            Err(err) => {
                return Err(RpcError::internal_error()
                    .with_message(err.to_string())
                    .into())
            }
        };

        *lock = Some(handle);
        Ok(())
    }

    async fn wait_for_task_result(
        &self,
        task_id: &str,
        session_id: Option<String>,
    ) -> SdkResult<(TaskStatus, Option<Res>)> {
        let rx_option = {
            let mut guard = self.inner.write().await;
            guard.subscribe_to_task(task_id, &session_id).await
        };

        let Some(rx) = rx_option else {
            return Err(RpcError::internal_error()
                .with_message("task does not exists!")
                .into());
        };

        match rx.await {
            Ok(result) => Ok(result),
            Err(err) => Err(RpcError::internal_error()
                .with_message(err.to_string())
                .into()),
        }
    }

    async fn get_task(&self, task_id: &str, session_id: Option<String>) -> Option<Task> {
        let inner = self.inner.read().await;
        inner
            .tasks
            .get(&session_id)
            .and_then(|map| map.get(task_id))
            .map(|entry| entry.task.clone())
    }

    async fn store_task_result(
        &self,
        task_id: &str,
        status: TaskStatus,
        result: Res,
        session_id: Option<&String>,
    ) -> () {
        let mut inner = self.inner.write().await;
        if let Some(session_map) = inner.tasks.get_mut(&session_id.map(|v| v.to_string())) {
            if let Some(entry) = session_map.get_mut(task_id) {
                let status_has_changed = entry.task.status != status;

                entry.task.status = status;
                entry.result = Some(result.clone());
                entry.task.last_updated_at = iso8601_time(current_utc_time(None));
                entry.task.status_message = None;
                tracing::debug!("Task result stored: {entry}");

                if status_has_changed {
                    self.notify_status_change(entry, session_id).await;
                }
            }
        }
    }

    async fn get_task_result(&self, task_id: &str, session_id: Option<String>) -> Option<Res> {
        let inner = self.inner.read().await;
        inner
            .tasks
            .get(&session_id)
            .and_then(|map| map.get(task_id))
            .and_then(|entry| entry.result.clone())
    }

    async fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        session_id: Option<String>,
    ) -> () {
        let mut inner = self.inner.write().await;
        if let Some(session_map) = inner.tasks.get_mut(&session_id) {
            if let Some(entry) = session_map.get_mut(task_id) {
                if entry.task.status != status {
                    self.notify_status_change(entry, session_id.as_ref()).await;
                }
                entry.task.status = status;
                entry.task.status_message = status_message;
                entry.task.last_updated_at = iso8601_time(current_utc_time(None));
                tracing::debug!("Task status updated: {entry}");
            }
        }
    }

    async fn list_tasks(
        &self,
        cursor: Option<String>,
        session_id: Option<String>,
    ) -> ListTasksResult {
        let inner = self.inner.read().await;
        let ordered_ids = match inner.ordered_task_ids.get(&session_id) {
            Some(ids) => ids,
            None => {
                return ListTasksResult {
                    tasks: vec![],
                    next_cursor: None,
                    meta: None,
                };
            }
        };

        let start_idx = cursor
            .as_ref()
            .and_then(|c| ordered_ids.iter().position(|id| id == c))
            .unwrap_or(0);
        let end_idx = (start_idx + self.page_size).min(ordered_ids.len());
        let page_ids = &ordered_ids[start_idx..end_idx];

        let tasks: Vec<Task> = page_ids
            .iter()
            .filter_map(|id| {
                inner
                    .tasks
                    .get(&session_id)
                    .and_then(|map| map.get(id))
                    .map(|entry| entry.task.clone())
            })
            .collect();

        let next_cursor = if end_idx < ordered_ids.len() {
            ordered_ids.get(end_idx).cloned()
        } else {
            None
        };

        ListTasksResult {
            tasks,
            next_cursor,
            meta: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::{advance, pause, resume};

    fn create_options(ttl_ms: Option<i64>) -> CreateTaskOptions {
        CreateTaskOptions {
            ttl: ttl_ms,
            poll_interval: Some(1000),
            meta: None,
        }
    }

    fn dummy_request() -> serde_json::Value {
        serde_json::json!({
            "method": "tools/call",
            "params": { "name": "test-tool" }
        })
    }

    #[tokio::test]
    async fn create_task_creates_with_working_status() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        let task = store
            .create_task(
                create_options(Some(60_000)),
                123.into(),
                dummy_request(),
                None,
            )
            .await;

        assert!(task.task_id.len() > 0);
        assert_eq!(task.status, TaskStatus::Working);
        assert_eq!(task.ttl, Some(60_000));
        assert!(task.poll_interval.is_some());
        assert!(task.created_at.len() > 0);
        assert!(task.last_updated_at.len() > 0);
    }

    #[tokio::test]
    async fn create_task_without_ttl() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        let task = store
            .create_task(create_options(None), 456.into(), dummy_request(), None)
            .await;

        assert_eq!(task.ttl, None);
    }

    #[tokio::test]
    async fn task_ids_are_unique() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        let task1 = store
            .create_task(create_options(None), 789.into(), dummy_request(), None)
            .await;
        let task2 = store
            .create_task(create_options(None), 790.into(), dummy_request(), None)
            .await;

        assert_ne!(task1.task_id, task2.task_id);
    }

    #[tokio::test]
    async fn get_task_returns_none_for_missing() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        let task = store.get_task("non-existent", None).await;
        assert!(task.is_none());
    }

    #[tokio::test]
    async fn update_and_get_task_status() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        let created = store
            .create_task(create_options(None), 111.into(), dummy_request(), None)
            .await;

        store
            .update_task_status(&created.task_id, TaskStatus::InputRequired, None, None)
            .await;

        let task = store.get_task(&created.task_id, None).await.unwrap();
        assert_eq!(task.status, TaskStatus::InputRequired);
    }

    #[tokio::test]
    async fn store_and_retrieve_task_result() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        let created = store
            .create_task(
                create_options(Some(60_000)),
                333.into(),
                dummy_request(),
                None,
            )
            .await;

        let result = serde_json::json!({
            "content": [{ "type": "text", "text": "Success!" }]
        });

        store
            .store_task_result(
                &created.task_id,
                TaskStatus::Completed,
                result.clone(),
                None,
            )
            .await;

        let task = store.get_task(&created.task_id, None).await.unwrap();
        assert_eq!(task.status, TaskStatus::Completed);

        let stored = store.get_task_result(&created.task_id, None).await;
        assert_eq!(stored, Some(result));
    }

    #[tokio::test]
    async fn ttl_expires_task_precisely() {
        pause(); // Make time controlled

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        let created = store
            .create_task(
                create_options(Some(1000)),
                666.into(),
                dummy_request(),
                None,
            )
            .await;

        let task = store.get_task(&created.task_id, None).await;
        assert!(task.is_some());

        advance_time_ms(10001).await;

        let task = store.get_task(&created.task_id, None).await;
        assert!(task.is_none());

        resume();
    }

    #[tokio::test]
    async fn tasks_without_ttl_do_not_expire() {
        pause();

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        let created = store
            .create_task(create_options(None), 888.into(), dummy_request(), None)
            .await;

        advance_time_ms(10001).await;

        let task = store.get_task(&created.task_id, None).await;
        assert!(task.is_some());

        resume();
    }

    async fn advance_time_ms(ms: u64) {
        tokio::task::yield_now().await;
        advance(Duration::from_millis(ms)).await;
        tokio::task::yield_now().await;
    }

    #[tokio::test]
    async fn completed_tasks_still_expire_after_ttl() {
        pause();
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        let created = store
            .create_task(
                create_options(Some(1000)),
                999.into(),
                dummy_request(),
                None,
            )
            .await;

        store
            .store_task_result(
                &created.task_id,
                TaskStatus::Completed,
                serde_json::json!({}),
                None,
            )
            .await;

        advance_time_ms(10001).await;

        let task = store.get_task(&created.task_id, None).await;

        assert!(task.is_none());

        resume();
    }

    #[tokio::test]
    async fn all_terminal_states_expire() {
        pause();

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        let working = store
            .create_task(
                create_options(Some(1000)),
                1001.into(),
                dummy_request(),
                None,
            )
            .await;

        let completed = store
            .create_task(
                create_options(Some(1000)),
                1002.into(),
                dummy_request(),
                None,
            )
            .await;
        store
            .store_task_result(
                &completed.task_id,
                TaskStatus::Completed,
                serde_json::json!({}),
                None,
            )
            .await;

        let failed = store
            .create_task(
                create_options(Some(1000)),
                1003.into(),
                dummy_request(),
                None,
            )
            .await;
        store
            .store_task_result(
                &failed.task_id,
                TaskStatus::Failed,
                serde_json::json!({ "is_error": true }),
                None,
            )
            .await;

        let cancelled = store
            .create_task(
                create_options(Some(1000)),
                1004.into(),
                dummy_request(),
                None,
            )
            .await;
        store
            .update_task_status(&cancelled.task_id, TaskStatus::Cancelled, None, None)
            .await;

        advance_time_ms(10001).await;

        assert!(store.get_task(&working.task_id, None).await.is_none());
        assert!(store.get_task(&completed.task_id, None).await.is_none());
        assert!(store.get_task(&failed.task_id, None).await.is_none());
        assert!(store.get_task(&cancelled.task_id, None).await.is_none());

        resume();
    }

    #[tokio::test]
    async fn list_tasks_pagination() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(Some(3)); // page size 3

        // Create 7 tasks (newest first)
        for i in 0..7 {
            store
                .create_task(create_options(None), i.into(), dummy_request(), None)
                .await;
        }

        let page1 = store.list_tasks(None, None).await;
        assert_eq!(page1.tasks.len(), 3);
        assert!(page1.next_cursor.is_some());

        let page2 = store.list_tasks(page1.next_cursor, None).await;
        assert_eq!(page2.tasks.len(), 3);
        assert!(page2.next_cursor.is_some());

        let page3 = store.list_tasks(page2.next_cursor, None).await;
        assert_eq!(page3.tasks.len(), 1);
        assert!(page3.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_tasks_empty() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        let result = store.list_tasks(None, None).await;
        assert_eq!(result.tasks.len(), 0);
        assert!(result.next_cursor.is_none());
    }

    #[tokio::test]
    async fn pagination_respects_order_newest_first() {
        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        let task1 = store
            .create_task(create_options(None), 1.into(), dummy_request(), None)
            .await;
        let task2 = store
            .create_task(create_options(None), 2.into(), dummy_request(), None)
            .await;
        let task3 = store
            .create_task(create_options(None), 3.into(), dummy_request(), None)
            .await;

        let list = store.list_tasks(None, None).await;
        let ids: Vec<_> = list.tasks.iter().map(|t| t.task_id.clone()).collect();

        // task3 should be first (newest)
        assert_eq!(ids[0], task3.task_id);
        assert_eq!(ids[1], task2.task_id);
        assert_eq!(ids[2], task1.task_id);
    }
}

#[cfg(test)]
mod polling_tests {
    use super::*;
    use crate::*;
    use rust_mcp_schema::RpcError;
    use serde_json::Value;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::time::{advance, pause};

    fn dummy_request() -> serde_json::Value {
        serde_json::json!({})
    }

    #[tokio::test]
    async fn new_with_polling_initializes_polling_schedule() {
        pause();

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);

        store.start_task_polling(Box::new(|task_id, _| {
            Box::pin(async { Ok((TaskStatus::Working, Some(500))) })
        }));

        let created = store
            .create_task(
                CreateTaskOptions {
                    ttl: None,
                    poll_interval: Some(500),
                    meta: None,
                },
                1.into(),
                dummy_request(),
                None,
            )
            .await;

        // Advance just past the first poll time
        advance(Duration::from_millis(600)).await;

        // Force one loop iteration by sleeping a bit (the spawned task runs in background)
        tokio::time::sleep(Duration::from_millis(100)).await;

        let inner = store.inner.read().await;
        assert!(inner.poll_schedule.is_some());
        let schedule = inner.poll_schedule.as_ref().unwrap();
        assert!(!schedule.is_empty(), "Heap should have scheduled the task");

        // The task should have been rescheduled
        let peeked = schedule.peek().unwrap();
        let (next_time, task_id, _) = &peeked.0;
        assert_eq!(task_id, &created.task_id);
        assert!(next_time > &Instant::now());
    }

    #[tokio::test]
    async fn new_with_polling_initializes_schedule_and_schedules_created_tasks() {
        let poll_count = Arc::new(Mutex::new(0));
        let count_clone = poll_count.clone();

        let callback: GetTaskCallback = Box::new(move |_task_id, _session_id| {
            let count = count_clone.clone();
            Box::pin(async move {
                *count.lock().await += 1;
                Ok((TaskStatus::Working, Some(150)))
            })
        });

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        store.start_task_polling(callback);

        // Create one task with short interval
        store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(150),
                    ttl: Some(60_000),
                    meta: None,
                },
                1.into(),
                dummy_request(),
                None,
            )
            .await;

        // Wait past the first poll
        tokio::time::sleep(Duration::from_millis(200)).await;

        let count = *poll_count.lock().await;
        assert!(count >= 1, "Task should have been polled at least once");

        // Wait a bit more — should be polled again
        tokio::time::sleep(Duration::from_millis(200)).await;
        let count2 = *poll_count.lock().await;
        assert!(
            count2 >= 2,
            "Task should have been rescheduled and polled again"
        );
    }

    #[tokio::test]
    async fn polling_respects_different_intervals_shortest_first() {
        let poll_order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = poll_order.clone();

        let callback: GetTaskCallback = Box::new(move |task_id, _session_id| {
            let order = order_clone.clone();
            Box::pin(async move {
                order.lock().await.push(task_id.clone());
                Ok((TaskStatus::Working, Some(200)))
            })
        });

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        store.start_task_polling(callback);

        // Create tasks: short, medium, long
        let task_short = store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(200),
                    ttl: Some(60_000),
                    meta: None,
                },
                1.into(),
                dummy_request(),
                None,
            )
            .await;

        let task_medium = store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(500),
                    ttl: Some(60_000),
                    meta: None,
                },
                2.into(),
                dummy_request(),
                None,
            )
            .await;

        let task_long = store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(1000),
                    ttl: Some(60_000),
                    meta: None,
                },
                3.into(),
                dummy_request(),
                None,
            )
            .await;

        // Wait just past shortest interval → only short should fire
        tokio::time::sleep(Duration::from_millis(250)).await;

        let order = poll_order.lock().await;
        assert_eq!(order.len(), 1);
        assert_eq!(order[0], task_short.task_id);
        drop(order);
        poll_order.lock().await.clear();

        // Wait more → short again (~400ms total), medium at 500ms
        tokio::time::sleep(Duration::from_millis(350)).await;

        let order = poll_order.lock().await.clone();
        assert_eq!(order.len(), 2);
        assert_eq!(order[0], task_short.task_id); // second poll of short
        assert_eq!(order[1], task_medium.task_id); // first poll of medium
        drop(order);
        poll_order.lock().await.clear();

        // Wait more → long at 1000ms, short again at ~600ms
        tokio::time::sleep(Duration::from_millis(500)).await;

        let order = poll_order.lock().await.clone();
        assert!(order.contains(&task_short.task_id));
        assert!(order.contains(&task_long.task_id));
    }

    #[tokio::test]
    async fn terminal_result_stops_rescheduling_that_task() {
        let poll_count = Arc::new(Mutex::new(0));
        let should_complete = Arc::new(Mutex::new(false));

        let count_clone = poll_count.clone();
        let complete_clone = should_complete.clone();

        let callback: GetTaskCallback = Box::new(move |task_id, _session_id| {
            let count = count_clone.clone();
            let complete = complete_clone.clone();
            Box::pin(async move {
                *count.lock().await += 1;
                let is_complete = *complete.lock().await;
                if is_complete {
                    Ok((TaskStatus::Completed, Some(200)))
                } else {
                    Ok((TaskStatus::Working, Some(200)))
                }
            })
        });

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        store.start_task_polling(callback);

        store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(200),
                    ttl: Some(60_000),
                    meta: None,
                },
                1.into(),
                dummy_request(),
                None,
            )
            .await;

        // First poll → Working
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(*poll_count.lock().await, 1);

        // Second poll → still Working
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(*poll_count.lock().await, 2);

        // Now make it return terminal on next poll
        *should_complete.lock().await = true;

        // Third poll → should return Completed and NOT be rescheduled
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(*poll_count.lock().await, 3);

        // Wait much longer — no more polls
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert_eq!(
            *poll_count.lock().await,
            3,
            "No further polling after terminal state"
        );
    }

    #[tokio::test]
    async fn error_in_callback_does_not_stop_rescheduling() {
        let poll_count = Arc::new(Mutex::new(0));
        let count_clone = poll_count.clone();

        let callback: GetTaskCallback = Box::new(move |_task_id, _session_id| {
            let count = count_clone.clone();
            Box::pin(async move {
                let mut c = count.lock().await;
                *c += 1;
                // Fail on the 3rd poll
                if *c == 3 {
                    Err(RpcError::internal_error()
                        .with_message("simulated failure")
                        .into())
                } else {
                    Ok((TaskStatus::Working, Some(200)))
                }
            })
        });

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        store.start_task_polling(callback);

        store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(200),
                    ttl: Some(60_000),
                    meta: None,
                },
                1.into(),
                dummy_request(),
                None,
            )
            .await;

        tokio::time::sleep(Duration::from_millis(220)).await;
        assert_eq!(*poll_count.lock().await, 1);

        tokio::time::sleep(Duration::from_millis(220)).await;
        assert_eq!(*poll_count.lock().await, 2);

        // This one fails
        tokio::time::sleep(Duration::from_millis(220)).await;
        assert_eq!(*poll_count.lock().await, 3);

        // But it should still be rescheduled
        tokio::time::sleep(Duration::from_millis(220)).await;
        assert_eq!(*poll_count.lock().await, 4);

        tokio::time::sleep(Duration::from_millis(220)).await;
        assert_eq!(*poll_count.lock().await, 5);
    }

    #[tokio::test]
    async fn multiple_tasks_with_varying_intervals_are_polled_correctly_over_time() {
        let poll_order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = poll_order.clone();

        let callback: GetTaskCallback = Box::new(move |task_id, _session_id| {
            let order = order_clone.clone();
            Box::pin(async move {
                order.lock().await.push(task_id.clone());
                Ok((TaskStatus::Working, Some(200)))
            })
        });

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        store.start_task_polling(callback);

        let task_a = store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(300),
                    ttl: Some(60_000),
                    meta: None,
                },
                1.into(),
                dummy_request(),
                None,
            )
            .await;
        let task_b = store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(500),
                    ttl: Some(60_000),
                    meta: None,
                },
                2.into(),
                dummy_request(),
                None,
            )
            .await;
        let task_c = store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(800),
                    ttl: Some(60_000),
                    meta: None,
                },
                3.into(),
                dummy_request(),
                None,
            )
            .await;

        // Let run for ~1.2 seconds
        tokio::time::sleep(Duration::from_millis(1200)).await;

        let order = poll_order.lock().await;
        let polls: std::collections::HashMap<_, usize> =
            order
                .iter()
                .fold(std::collections::HashMap::new(), |mut acc, id| {
                    *acc.entry(id).or_insert(0) += 1;
                    acc
                });

        // task_a (300ms): should be polled ~4 times (300,600,900,1200)
        assert!(polls[&task_a.task_id] >= 3);

        // task_b (500ms): ~2-3 times
        assert!(polls[&task_b.task_id] >= 2);

        // task_c (800ms): ~1-2 times
        assert!(polls[&task_c.task_id] >= 1);
    }

    #[tokio::test]
    async fn await_for_task_result() {
        let poll_count = Arc::new(Mutex::new(0));
        let count_clone = poll_count.clone();

        let callback: GetTaskCallback = Box::new(move |_task_id, _session_id| {
            let count = count_clone.clone();
            Box::pin(async move {
                *count.lock().await += 1;
                Ok((TaskStatus::Completed, Some(150)))
            })
        });

        let store = InMemoryTaskStore::<serde_json::Value, serde_json::Value>::new(None);
        store.start_task_polling(callback).unwrap();

        // Create one task with short interval
        let task = store
            .create_task(
                CreateTaskOptions {
                    poll_interval: Some(150),
                    ttl: Some(60_000),
                    meta: None,
                },
                1.into(),
                dummy_request(),
                None,
            )
            .await;
        store
            .store_task_result(
                &task.task_id,
                TaskStatus::Completed,
                Value::from("task result"),
                None,
            )
            .await;
        let result = store
            .wait_for_task_result(&task.task_id, None)
            .await
            .unwrap();

        assert_eq!(result.0, TaskStatus::Completed);
        assert_eq!(result.1, Some(Value::from("task result")));
    }
}
