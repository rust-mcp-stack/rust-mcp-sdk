use super::{CreateTaskOptions, TaskStore};
use crate::id_generator::FastIdGenerator;
use crate::task_store::TaskStatusSignal;
use crate::utils::{current_utc_time, iso8601_time};
use crate::IdGenerator;
use async_trait::async_trait;
use futures::{stream, Stream};
use rust_mcp_schema::{ListTasksResult, RequestId, Task, TaskStatus, TaskStatusNotificationParams};
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Display};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const DEFAULT_PAGE_SIZE: usize = 50;

#[derive(Clone)]
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
            })),
            broadcast: tokio::sync::broadcast::channel(64).0,
            page_size: page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            id_gen: Arc::new(FastIdGenerator::new(Some("tsk"))),
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
        };

        tracing::debug!("New task created: {entry}");

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
        session_id: Option<String>,
    ) -> () {
        let mut inner = self.inner.write().await;
        if let Some(session_map) = inner.tasks.get_mut(&session_id) {
            if let Some(entry) = session_map.get_mut(task_id) {
                entry.task.status = status;
                entry.result = Some(result);
                entry.task.last_updated_at = iso8601_time(current_utc_time(None));
                entry.task.status_message = None;
                tracing::debug!("Task result stored: {entry}");
                self.notify_status_change(entry, session_id.as_ref()).await;
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
