mod in_memory_task_store;
use async_trait::async_trait;
use futures::Stream;
pub use in_memory_task_store::*;
use rust_mcp_schema::{
    schema_utils::{
        ClientJsonrpcRequest, ResultFromClient, ResultFromServer, ServerJsonrpcRequest,
    },
    ListTasksResult, RequestId, Task, TaskStatus, TaskStatusNotificationParams,
};
use std::{fmt::Debug, pin::Pin, sync::Arc};

use crate::error::SdkResult;

/// A stream of task status notifications, where each item contains the notification parameters
/// and an optional session_id
pub type TaskStatusStream =
    Pin<Box<dyn Stream<Item = (TaskStatusNotificationParams, Option<String>)> + Send + 'static>>;

#[async_trait]
pub trait TaskStatusSignal: Send + Sync + 'static {
    /// Publish a status change event
    async fn publish_status_change(
        &self,
        event: TaskStatusNotificationParams,
        session_id: Option<&String>,
    );
    /// Return a new independent stream of events
    fn subscribe(&self) -> Option<TaskStatusStream> {
        None
    }
}

pub type TaskStatusCallback = Box<dyn Fn(&Task, Option<&String>) + Send + Sync + 'static>;

pub struct CreateTaskOptions {
    ///Actual retention duration from creation in milliseconds, None for unlimited.
    pub ttl: Option<i64>,
    pub poll_interval: ::std::option::Option<i64>,
    ///Additional context to pass to the task store.
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
    // pub context: Option<HashMap<String, Box<dyn Any + Send>>>,
}

pub struct TaskCreator<Req, Res>
where
    Req: Debug + Clone + serde::Deserialize<'static> + serde::Serialize,
    Res: Debug + Clone + serde::Deserialize<'static> + serde::Serialize,
{
    pub request_id: RequestId,
    pub request: Req,
    pub session_id: Option<String>,
    pub task_store: Arc<dyn TaskStore<Req, Res>>,
}

impl<Req, Res> TaskCreator<Req, Res>
where
    Req: Debug + Clone + serde::Deserialize<'static> + serde::Serialize + 'static,
    Res: Debug + Clone + serde::Deserialize<'static> + serde::Serialize + 'static,
{
    pub async fn create_task(self, task_params: CreateTaskOptions) -> Task {
        self.task_store
            .create_task(task_params, self.request_id, self.request, self.session_id)
            .await
    }
}

/// A trait for storing and managing long-running tasks, storing and retrieving task state and results.
/// Tasks were introduced in MCP Protocol version 2025-11-25.
/// For more details, see: <https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks>
#[async_trait]
pub trait TaskStore<Req, Res>: Send + Sync + TaskStatusSignal
where
    Req: Debug + Clone + serde::Deserialize<'static> + serde::Serialize,
    Res: Debug + Clone + serde::Deserialize<'static> + serde::Serialize,
{
    /// Creates a new task with the given creation parameters and original request.
    /// The implementation must generate a unique taskId and createdAt timestamp.
    ///
    /// TTL Management:
    /// - The implementation receives the TTL suggested by the requestor via taskParams.ttl
    /// - The implementation MAY override the requested TTL (e.g., to enforce limits)
    /// - The actual TTL used MUST be returned in the Task object
    /// - Null TTL indicates unlimited task lifetime (no automatic cleanup)
    /// - Cleanup SHOULD occur automatically after TTL expires, regardless of task status
    ///
    /// # Arguments
    /// * `task_params` - The task creation parameters from the request (ttl, pollInterval)
    /// * `request_id` - The JSON-RPC request ID
    /// * `request` - The original request that triggered task creation
    /// * `session_id` - Optional session ID for binding the task to a specific session
    ///
    /// # Returns
    /// The created task object
    async fn create_task(
        &self,
        task_params: CreateTaskOptions,
        request_id: RequestId,
        request: Req,
        session_id: Option<String>,
    ) -> Task;

    /// Begins active polling for task status updates in requestor mode.
    /// This method spawns a long-running background task that drives the polling
    /// schedule for all tasks managed by this store. It repeatedly invokes the
    /// provided `get_task_callback` to query the **receiver** for the current status
    /// of pending tasks.
    ///
    /// The polling loop should respect the `pollInterval` suggested by the receiver and
    /// dynamically adjusts accordingly. Each task is polled until it reaches a
    /// terminal state (`Completed`, `Failed`, or `Cancelled`), at which point it
    /// is removed from the active polling schedule.
    ///
    /// This mechanism is used when the local side acts as the **requestor** in the
    /// Model Context Protocol task flow — i.e., when a task-augmented request has
    /// been sent to the remote side (the receiver) and the local side needs to
    /// actively monitor progress via repeated `tasks/get` calls.
    fn start_task_polling(&self, get_task_callback: GetTaskCallback) -> SdkResult<()>;

    /// Waits asynchronously for the result of a task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The unique identifier of the task whose result is awaited.
    /// * `session_id` - Optional session identifier used to disambiguate or scope the task.
    ///
    /// # Returns
    ///
    /// * `Ok(Res)` if the task completes successfully and sends its result.
    /// * `Err(SdkError)` if:
    ///   - the task does not exist,
    ///   - the task result channel is dropped before sending,
    ///   - or an internal error occurs.
    ///
    /// # Errors
    ///
    /// Returns an internal RPC error if the task does not exist or if the sender
    /// side of the oneshot channel is dropped before producing a result.
    async fn wait_for_task_result(
        &self,
        task_id: &str,
        session_id: Option<String>,
    ) -> SdkResult<(TaskStatus, Option<Res>)>;

    /// Gets the current status of a task.
    ///
    /// # Arguments
    /// * `task_id` - The task identifier
    /// * `session_id` - Optional session ID for binding the query to a specific session
    ///
    /// # Returns
    /// The task object, or None if it does not exist
    async fn get_task(&self, task_id: &str, session_id: Option<String>) -> Option<Task>;

    /// Stores the result of a task and sets its final status.
    ///
    /// # Arguments
    /// * `task_id` - The task identifier
    /// * `status` - The final status: 'completed' for success, 'failed' for errors
    /// * `result` - The result to store
    /// * `session_id` - Optional session ID for binding the operation to a specific session
    async fn store_task_result(
        &self,
        task_id: &str,
        status: TaskStatus,
        result: Res,
        session_id: Option<&String>,
    ) -> ();

    /// Retrieves the stored result of a task.
    ///
    /// # Arguments
    /// * `task_id` - The task identifier
    /// * `session_id` - Optional session ID for binding the query to a specific session
    ///
    /// # Returns
    /// The stored result
    async fn get_task_result(&self, task_id: &str, session_id: Option<String>) -> Option<Res>;

    /// Updates a task's status (e.g., to 'cancelled', 'failed', 'completed').
    ///
    /// # Arguments
    /// * `task_id` - The task identifier
    /// * `status` - The new status
    /// * `status_message` - Optional diagnostic message for failed tasks or other status information
    /// * `session_id` - Optional session ID for binding the operation to a specific session
    async fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        session_id: Option<String>,
    ) -> ();

    /// Lists tasks, optionally starting from a pagination cursor.
    ///
    /// # Arguments
    /// * `cursor` - Optional cursor for pagination
    /// * `session_id` - Optional session ID for binding the query to a specific session
    ///
    /// # Returns
    /// An object containing the tasks array and an optional nextCursor
    async fn list_tasks(
        &self,
        cursor: Option<String>,
        session_id: Option<String>,
    ) -> ListTasksResult;
}

pub type ServerTaskCreator = TaskCreator<ClientJsonrpcRequest, ResultFromServer>;
pub type ClientTaskCreator = TaskCreator<ServerJsonrpcRequest, ResultFromClient>;

pub type ServerTaskStore = dyn TaskStore<ClientJsonrpcRequest, ResultFromServer>;
pub type ClientTaskStore = dyn TaskStore<ServerJsonrpcRequest, ResultFromClient>;
