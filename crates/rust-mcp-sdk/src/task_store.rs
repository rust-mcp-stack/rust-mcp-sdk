// mod in_memory_task_store;
// pub use in_memory_task_store::*;

use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{RequestFromClient, RequestFromServer, ResultFromClient, ResultFromServer},
    ListTasksResult, RequestId, Task, TaskStatus,
};

#[derive(::serde::Deserialize, ::serde::Serialize, Debug)]
pub struct CreateTaskOptions {
    ///Actual retention duration from creation in milliseconds, null for unlimited.
    pub ttl: i64,
    ///Suggested polling interval in milliseconds.
    #[serde(
        rename = "pollInterval",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub poll_interval: ::std::option::Option<i64>,
    ///Additional context to pass to the task store.
    pub context: Option<serde_json::Map<String, serde_json::Value>>,
}

/// A trait for storing and managing long-running tasks, storing and retrieving task state and results.
/// Tasks were introduced in MCP Protocol version 2025-11-25.
/// For more details, see: https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks
#[async_trait]
pub trait TaskStore<Req, Res>: Send + Sync {
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
        session_id: Option<String>,
    ) -> ();

    /// Retrieves the stored result of a task.
    ///
    /// # Arguments
    /// * `task_id` - The task identifier
    /// * `session_id` - Optional session ID for binding the query to a specific session
    ///
    /// # Returns
    /// The stored result
    async fn get_task_result(&self, task_id: &str, session_id: Option<String>) -> Res;

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

    /// Optionally registers a callback to be invoked whenever a task's status changes.
    ///
    /// The callback, if provided, receives the task ID and the new status.
    fn on_status_change(&self, _callback: Box<dyn Fn(&str, &TaskStatus) + Send + Sync + 'static>) {
        unimplemented!("on_status_change is not implemented");
    }
}

pub type ServerTaskStore = dyn TaskStore<RequestFromClient, ResultFromServer>;
pub type ClientTaskStore = dyn TaskStore<RequestFromServer, ResultFromClient>;
