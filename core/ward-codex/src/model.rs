// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::num::NonZeroU64;
use std::path::PathBuf;

/// Information returned by the app-server initialization handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerInfo {
    pub codex_home: PathBuf,
    pub platform_family: String,
    pub platform_os: String,
    pub user_agent: String,
}

/// The complete visible model catalog advertised by one Codex app-server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCatalog {
    pub models: Vec<ModelInfo>,
}

/// One model available for Codex conversations and turns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInfo {
    /// Catalog identity assigned by the app-server.
    pub id: String,
    /// Canonical model value accepted by thread and turn requests.
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub is_default: bool,
    pub default_reasoning_effort: ReasoningEffort,
    pub supported_reasoning_efforts: Vec<ReasoningEffortOption>,
}

impl ModelInfo {
    pub(crate) fn resolve_reasoning_effort(
        &self,
        preferred_effort: Option<&str>,
    ) -> ReasoningEffort {
        preferred_effort
            .and_then(|preferred| {
                self.supported_reasoning_efforts
                    .iter()
                    .find(|option| option.effort.as_str() == preferred)
                    .map(|option| option.effort.clone())
            })
            .unwrap_or_else(|| self.default_reasoning_effort.clone())
    }
}

/// A non-empty reasoning-effort value advertised by a Codex model.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReasoningEffort(String);

impl ReasoningEffort {
    /// Creates a reasoning effort when the supplied value is non-empty.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()).then_some(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One reasoning-effort choice supported by a Codex model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningEffortOption {
    pub effort: ReasoningEffort,
    pub description: String,
}

/// One requested change to a thread's active inference options.
///
/// Model-only overrides are resolved against the advertised model catalog so
/// the client can report the reasoning effort that actually became active.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferenceOverride {
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
}

impl InferenceOverride {
    #[must_use]
    pub fn model(model: impl Into<String>) -> Self {
        Self {
            model: Some(model.into()),
            reasoning_effort: None,
        }
    }

    #[must_use]
    pub fn reasoning_effort(reasoning_effort: ReasoningEffort) -> Self {
        Self {
            model: None,
            reasoning_effort: Some(reasoning_effort),
        }
    }

    #[must_use]
    pub fn selection(model: impl Into<String>, reasoning_effort: ReasoningEffort) -> Self {
        Self {
            model: Some(model.into()),
            reasoning_effort: Some(reasoning_effort),
        }
    }

    pub(crate) fn model_override(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub(crate) fn reasoning_effort_override(&self) -> Option<&ReasoningEffort> {
        self.reasoning_effort.as_ref()
    }

    pub(crate) fn set_reasoning_effort(&mut self, reasoning_effort: ReasoningEffort) {
        self.reasoning_effort = Some(reasoning_effort);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ThreadInferenceState {
    model: Option<String>,
    reasoning_effort: Option<String>,
}

impl ThreadInferenceState {
    pub(crate) fn new(model: Option<String>, reasoning_effort: Option<String>) -> Self {
        Self {
            model,
            reasoning_effort,
        }
    }

    pub(crate) fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub(crate) fn reasoning_effort(&self) -> Option<&str> {
        self.reasoning_effort.as_deref()
    }

    pub(crate) fn apply(&mut self, inference_override: &InferenceOverride) {
        if let Some(model) = inference_override.model_override() {
            self.model = Some(model.to_owned());
        }
        if let Some(reasoning_effort) = inference_override.reasoning_effort_override() {
            self.reasoning_effort = Some(reasoning_effort.to_string());
        }
    }
}

/// Collaboration behavior selected for one Codex turn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum TurnMode {
    #[default]
    Default,
    Plan,
}

/// Permission behavior selected for one Codex turn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum TurnPermissionPreset {
    /// Preserve the permission settings already associated with the thread.
    #[default]
    Inherit,
    /// Allow workspace edits while requesting approval for sandbox escalation.
    RequestApproval,
    /// Keep the turn read-only unless the user approves an escalation.
    ReadOnly,
}

/// One user-supplied input item used to start a Codex turn.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TurnInput {
    Text(String),
    LocalImage { path: PathBuf },
    LocalAudio { path: PathBuf },
    Mention { name: String, path: PathBuf },
}

/// User-facing controls that affect how a Codex turn runs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TurnOptions {
    pub mode: TurnMode,
    pub permission_preset: TurnPermissionPreset,
    /// Override the thread's active inference options for this turn and
    /// subsequent turns.
    pub inference: Option<InferenceOverride>,
}

/// Controls how a new Codex thread is created.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ThreadStartOptions {
    /// Keep the thread in memory without materializing it in persisted history.
    pub ephemeral: bool,
}

/// A page of Codex thread summaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadPage {
    pub threads: Vec<ThreadSummary>,
    pub next_cursor: Option<String>,
}

/// Metadata sufficient to render a thread in a history list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadSummary {
    pub id: String,
    pub name: Option<String>,
    pub preview: String,
    pub cwd: PathBuf,
    pub created_at_unix_seconds: i64,
    pub updated_at_unix_seconds: i64,
}

/// A Codex thread with its persisted turns loaded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thread {
    pub summary: ThreadSummary,
    pub turns: Vec<Turn>,
}

/// A thread snapshot returned after subscribing a writer connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadSubscription {
    pub thread: Thread,
    pub runtime_status: ThreadRuntimeStatus,
}

/// Runtime status reported by the app-server process that owns a subscription.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadRuntimeStatus {
    NotLoaded,
    Idle,
    Active { active_flags: Vec<ThreadActiveFlag> },
    SystemError,
    Unknown(String),
}

/// A condition that explains why an active thread is waiting.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadActiveFlag {
    WaitingOnApproval,
    WaitingOnUserInput,
    Unknown(String),
}

/// Opaque non-zero identifier assigned to one app-server request awaiting user
/// input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InteractionId(NonZeroU64);

impl InteractionId {
    /// Creates an interaction identifier from its private wire representation.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the private wire representation of this identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for InteractionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One app-server request that Craftward can present and answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingInteraction {
    pub id: InteractionId,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub kind: PendingInteractionKind,
    pub command: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub reason: Option<String>,
    pub grant_root: Option<PathBuf>,
    pub available_decisions: Vec<InteractionDecision>,
    pub questions: Vec<InteractionQuestion>,
    pub user_input_is_blocking: bool,
}

/// User-facing category of a pending app-server request.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PendingInteractionKind {
    CommandApproval,
    FileChangeApproval,
    UserInput,
}

/// A decision accepted by a command or file-change approval request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InteractionDecision {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

/// One question included in a Codex request for user input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionQuestion {
    pub id: String,
    pub header: String,
    pub prompt: String,
    pub options: Vec<InteractionOption>,
    pub allows_other: bool,
    pub secret: bool,
}

/// One selectable answer advertised for a user-input question.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionOption {
    pub label: String,
    pub description: String,
}

/// One answer supplied for a user-input question.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionAnswer {
    pub question_id: String,
    pub answers: Vec<String>,
}

/// A response to one pending interaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionResponse {
    pub interaction_id: InteractionId,
    pub body: InteractionResponseBody,
}

/// The typed payload used to resolve a pending interaction.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InteractionResponseBody {
    Decision(InteractionDecision),
    UserInput(Vec<InteractionAnswer>),
}

/// One persisted turn in a Codex thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Turn {
    pub id: String,
    pub status: TurnStatus,
    pub items: Vec<ThreadItem>,
}

/// One streamed update emitted on a subscribed Codex thread connection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadStreamEvent {
    ThreadStatusChanged {
        thread_id: String,
        status: ThreadRuntimeStatus,
    },
    TurnStarted {
        thread_id: String,
        turn: Turn,
    },
    ItemStarted {
        thread_id: String,
        turn_id: String,
        item: ThreadItem,
    },
    ItemCompleted {
        thread_id: String,
        turn_id: String,
        item: ThreadItem,
    },
    AgentMessageDelta {
        thread_id: String,
        turn_id: String,
        item_id: String,
        delta: String,
    },
    ActivityOutputDelta {
        thread_id: String,
        turn_id: String,
        item_id: String,
        delta: String,
    },
    ActivityUpdated {
        thread_id: String,
        turn_id: String,
        item_id: String,
        update: ActivityUpdate,
    },
    RuntimeError {
        thread_id: String,
        turn_id: String,
        message: String,
        will_retry: bool,
    },
    TurnCompleted {
        thread_id: String,
        turn: Turn,
    },
    PendingInteractionsUpdated {
        thread_id: String,
        interactions: Vec<PendingInteraction>,
    },
    UnsupportedServerRequest {
        thread_id: Option<String>,
        method: String,
    },
}

/// A normalized incremental change to one live activity item.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ActivityUpdate {
    AppendSummary(String),
    StartSummarySection,
    AppendDetail(String),
    ReplaceContent {
        summary: String,
        detail: Option<String>,
    },
}

/// The lifecycle status recorded for a turn.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TurnStatus {
    Completed,
    Interrupted,
    Failed,
    InProgress,
    Unknown(String),
}

impl TurnStatus {
    /// Returns whether the persisted turn has reached a known terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Interrupted | Self::Failed)
    }

    /// Returns whether the persisted turn is explicitly still in progress.
    #[must_use]
    pub fn is_in_progress(&self) -> bool {
        matches!(self, Self::InProgress)
    }
}

/// A persisted item relevant to rendering a conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadItem {
    UserMessage {
        id: String,
        content: Vec<UserInput>,
    },
    AgentMessage {
        id: String,
        text: String,
        phase: Option<AgentMessagePhase>,
    },
    Activity(Activity),
    Other {
        id: String,
        kind: String,
    },
}

/// One normalized activity retained in a persisted Codex turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Activity {
    pub id: String,
    pub kind: ActivityKind,
    pub status: ActivityStatus,
    pub started_at_unix_milliseconds: Option<i64>,
    pub completed_at_unix_milliseconds: Option<i64>,
    pub summary: String,
    pub detail: Option<String>,
    pub context: Option<String>,
    pub command_actions: Vec<CommandAction>,
}

/// The user-facing category of a retained Codex activity.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ActivityKind {
    Reasoning,
    Plan,
    CommandExecution,
    FileChange,
    ToolCall,
    Collaboration,
    WebSearch,
    ImageView,
    Wait,
    ImageGeneration,
    ReviewStarted,
    ReviewCompleted,
    ContextCompaction,
}

/// The lifecycle status retained for an activity, when supplied by Codex.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ActivityStatus {
    Unspecified,
    InProgress,
    Completed,
    Failed,
    Declined,
    Unknown(String),
}

/// A best-effort semantic action parsed from a shell command by Codex.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandAction {
    pub kind: CommandActionKind,
    pub command: String,
    pub name: Option<String>,
    pub path: Option<PathBuf>,
    pub query: Option<String>,
}

/// The semantic category assigned to a parsed command action.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommandActionKind {
    Read,
    ListFiles,
    Search,
    Unknown,
}

/// Content attached to a persisted user message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UserInput {
    Text(String),
    Image { url: String },
    LocalImage { path: PathBuf },
    Audio { url: String },
    LocalAudio { path: PathBuf },
    Skill { name: String, path: PathBuf },
    Mention { name: String, path: PathBuf },
    Other { kind: String },
}

/// The presentation phase of an assistant message, when supplied by Codex.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AgentMessagePhase {
    Commentary,
    FinalAnswer,
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::TurnStatus;

    #[test]
    fn classifies_known_and_unknown_turn_statuses() {
        for terminal in [
            TurnStatus::Completed,
            TurnStatus::Interrupted,
            TurnStatus::Failed,
        ] {
            assert!(terminal.is_terminal());
            assert!(!terminal.is_in_progress());
        }

        assert!(!TurnStatus::InProgress.is_terminal());
        assert!(TurnStatus::InProgress.is_in_progress());

        let unknown = TurnStatus::Unknown("futureStatus".to_owned());
        assert!(!unknown.is_terminal());
        assert!(!unknown.is_in_progress());
    }
}
