use super::{CreateTaskOptions, TaskStore};
use crate::id_generator::FastIdGenerator;
use crate::utils::{current_utc_time, iso8601_time};
use crate::IdGenerator;
use async_trait::async_trait;
use rust_mcp_schema::{ListTasksResult, RequestId, Task, TaskStatus};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
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
    inner: Arc<RwLock<InMemoryTaskStoreInner<Req, Res>>>,
    page_size: usize,
    // Optional background cleanup task handle
    _cleanup_handle: Arc<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
struct TaskEntry<Req, Res> {
    task: Task,
    request: Req,            // original request that created the task
    result: Option<Res>,     // stored only after store_task_result
    expires_at: Option<i64>, // Unix millis, for TTL cleanup
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
    Req: Clone + Send + Sync + 'static,
    Res: Clone + Send + Sync + 'static,
{
    pub fn new(page_size: Option<usize>) -> Self {
        let inner = Arc::new(RwLock::new(InMemoryTaskStoreInner {
            tasks: HashMap::new(),
            ordered_task_ids: HashMap::new(),
        }));

        // Spawn background cleanup task
        let inner_clone = inner.clone();
        let cleanup_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;

                let now = current_utc_time(None).unix_timestamp();

                let mut write_guard = inner_clone.write().await;
                for task_map in write_guard.tasks.values_mut() {
                    let expired: Vec<String> = task_map
                        .iter()
                        .filter(|(_, entry)| entry.expires_at.map_or(false, |exp| now >= exp))
                        .map(|(id, _)| id.clone())
                        .collect();

                    for id in expired {
                        task_map.remove(&id);
                    }
                }

                // Also clean up empty ordered lists
                write_guard
                    .ordered_task_ids
                    .retain(|_, ids| !ids.is_empty());
            }
        });

        Self {
            id_gen: Arc::new(FastIdGenerator::new(Some("tsk"))),
            inner,
            page_size: page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            _cleanup_handle: Arc::new(cleanup_handle),
        }
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
        let expires_at = task_params.ttl.map(|ttl| current_utc_time(Some(ttl)));

        let task = Task {
            task_id: task_id.clone(),
            created_at: created_at.clone(),
            status: TaskStatus::Working,
            poll_interval: task_params.poll_interval,
            ttl: task_params.ttl,
            status_message: None,
            last_updated_at: created_at,
        };

        let entry = TaskEntry {
            task: task.clone(),
            request,
            result: None,
            expires_at: expires_at.map(|t| t.unix_timestamp()),
        };

        tracing::debug!("New task created: {:?}", entry);

        // Insert into tasks map
        let session_tasks = inner
            .tasks
            .entry(session_id.clone())
            .or_insert_with(BTreeMap::new);
        session_tasks.insert(task_id.clone(), entry);

        // Insert into ordered list (newest first)
        let ordered = inner
            .ordered_task_ids
            .entry(session_id)
            .or_insert_with(Vec::new);
        ordered.insert(0, task_id); // newest at front

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
                entry.task.status = status.clone();
                entry.result = Some(result);
            }
        }
    }

    async fn get_task_result(&self, task_id: &str, session_id: Option<String>) -> Option<Res> {
        let inner = self.inner.read().await;
        inner
            .tasks
            .get(&session_id)
            .and_then(|map| map.get(task_id))
            .and_then(|entry| entry.result.to_owned())
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
                entry.task.status = status;
                entry.task.status_message = status_message;
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
                }
            }
        };

        // Simple cursor-based pagination: cursor is the last task_id from previous page
        let start_idx = cursor
            .as_ref()
            .and_then(|c| ordered_ids.iter().position(|id| id == c))
            .map(|pos| pos + 1)
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

// Default implementation
impl<Req, Res> Default for InMemoryTaskStore<Req, Res>
where
    Req: Clone + Send + Sync + 'static,
    Res: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new(Some(DEFAULT_PAGE_SIZE))
    }
}
